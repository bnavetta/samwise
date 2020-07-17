//! Wake-on-LAN implementation

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use pnet::datalink::{self, Channel, DataLinkSender, MacAddr, NetworkInterface};
use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::MutablePacket;

/// Size of a magic packet, including Ethernet headers
const MAGIC_PACKET_SIZE: usize = 116;

/// Wakes up remote devices via Wake-on-LAN
#[derive(Clone)]
pub struct Waker {
    shared: Arc<Shared>,
}

struct Shared {
    interfaces: Vec<NetworkInterface>,
    /// Memoized per-interface state
    senders: Mutex<HashMap<String, Arc<Mutex<WolSender>>>>,
}

impl Shared {
    /// Get a network interface by name
    fn interface(&self, name: &str) -> Option<&NetworkInterface> {
        self.interfaces
            .iter()
            .find(|interface| interface.name == name)
    }

    /// Get the `WolSender` for a network interface, creating it if necessary.
    fn sender(&self, interface: &str) -> Result<Arc<Mutex<WolSender>>> {
        let mut senders = self
            .senders
            .lock()
            .expect("Thread panicked with senders mutex");

        match senders.get(interface) {
            Some(sender) => Ok(sender.clone()),
            None => {
                let sender = match self.interface(interface) {
                    Some(interface) => Arc::new(Mutex::new(WolSender::new(interface)?)),
                    None => bail!("No such network interface: {}", interface),
                };
                senders.insert(interface.to_string(), sender.clone());
                Ok(sender)
            }
        }
    }
}

impl Waker {
    pub fn new() -> Waker {
        Waker {
            shared: Arc::new(Shared {
                interfaces: datalink::interfaces(),
                senders: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Wake a device by sending it a Wake-on-LAN magic packet.
    pub async fn wake(&self, interface: String, address: MacAddr) -> Result<()> {
        let shared = self.shared.clone(); // Clone here to avoid self needing a 'static lifetime
        tokio::task::spawn_blocking(move || {
            // Get sender inside spawn_blocking since creating it may be expensive
            let sender_mutex = shared.sender(&interface)?;
            let mut sender = sender_mutex
                .lock()
                .expect("Thread panicked with sender mutex");
            sender.send_magic_packet(address)
        })
        .await?
    }
}

/// Per-interface state for sending magic packets
struct WolSender {
    source_address: MacAddr,
    datalink_tx: Box<dyn DataLinkSender>,
}

impl WolSender {
    fn new(interface: &NetworkInterface) -> Result<WolSender> {
        let datalink_tx = match datalink::channel(interface, Default::default()) {
            Ok(Channel::Ethernet(tx, _)) => tx,
            Ok(_) => bail!("Unknown channel type"),
            Err(e) => return Err(e).context("Could not create datalink channel"),
        };

        let source_address = match interface.mac {
            Some(mac) => mac,
            None => bail!(
                "Network interface {} does not have a MAC address",
                interface.name
            ),
        };

        Ok(WolSender {
            source_address,
            datalink_tx,
        })
    }

    fn send_magic_packet(&mut self, to: MacAddr) -> Result<()> {
        let source = self.source_address.clone();
        self.datalink_tx
            .build_and_send(1, MAGIC_PACKET_SIZE, &mut |buf| {
                // Panicking because an error means pnet didn't meet the build_and_send contract
                // and gave us a buffer of the wrong size
                build_magic_packet(source, to, buf).expect("Magic packet construction failed");
            })
            .context("Datalink buffer too small")?
            .with_context(|| format!("Sending magic packet to {} failed", to))?;

        Ok(())
    }
}

/// Writes a Wake-on-LAN magic packet for `dest` into `buf`. Fails if `buf` is not the correct magic packet size.
fn build_magic_packet(source: MacAddr, dest: MacAddr, buf: &mut [u8]) -> Result<()> {
    if buf.len() != MAGIC_PACKET_SIZE {
        bail!(
            "Invalid packet length: was {}, expected {}",
            buf.len(),
            MAGIC_PACKET_SIZE
        );
    }

    let mut packet = match MutableEthernetPacket::new(buf) {
        Some(packet) => packet,
        // Should be unreachable because of the length check above
        None => unreachable!("Buffer was too small for Ethernet packet"),
    };

    packet.set_ethertype(EtherTypes::WakeOnLan);
    packet.set_source(source);
    packet.set_destination(dest);

    let payload = packet.payload_mut();
    // First 6 bytes of the magic packet are 0xff
    for byte in &mut payload[0..6] {
        *byte = 0xff;
    }

    // Followed by 16 repetitions of the destination MAC address
    for i in 1..17 {
        let offset = i * 6;
        payload[offset] = dest.0;
        payload[offset + 1] = dest.1;
        payload[offset + 2] = dest.2;
        payload[offset + 3] = dest.3;
        payload[offset + 4] = dest.4;
        payload[offset + 5] = dest.5;
    }

    Ok(())
}
