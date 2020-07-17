use std::str::FromStr;

use pnet::datalink::MacAddr;
use pnet::datalink::{self, Channel};
use pnet::packet::ethernet::{EtherTypes, MutableEthernetPacket};
use pnet::packet::MutablePacket;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Args {
    pub interface: String,
    pub destination: String,
}

fn main() {
    let args: Args = Args::from_args();

    let interface = datalink::interfaces()
        .into_iter()
        .find(|iface| iface.name == args.interface)
        .expect("Could not find network interface");

    let destination =
        MacAddr::from_str(&args.destination).expect("Invalid destination MAC address");

    let (mut tx, _) = match datalink::channel(&interface, Default::default()) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unhandled channel type!"),
        Err(e) => panic!("Error creating datalink channel: {}", e),
    };

    tx.build_and_send(1, 116, &mut |bytes| {
        let mut packet = MutableEthernetPacket::new(bytes).unwrap();
        packet.set_ethertype(EtherTypes::WakeOnLan);
        packet.set_source(interface.mac.unwrap());
        packet.set_destination(destination);

        let payload: &mut [u8] = packet.payload_mut();
        // First 6 bytes of the magic packet are 0xff
        for byte in &mut payload[0..6] {
            *byte = 0xff;
        }
        // Followed by 16 repetitions of the destination MAC address
        for i in 1..17 {
            let offset = i * 6;
            payload[offset] = destination.0;
            payload[offset + 1] = destination.1;
            payload[offset + 2] = destination.2;
            payload[offset + 3] = destination.3;
            payload[offset + 4] = destination.4;
            payload[offset + 5] = destination.5;
        }
    })
    .unwrap()
    .unwrap();
}

// use eui48::MacAddress;
// use std::net::SocketAddr;
// use tokio::net::UdpSocket;
//
// #[tokio::main]
// async fn main() {
//     let mac_addr = MacAddress::parse_str("b4:2e:99:a0:f3:27").unwrap();
//
//     let mut magic_packet = [0xffu8; 102];
//     for i in 0..16 {
//         let offset = (i + 1) * 6; // skip first 6 bytes for 0xFFs, then each repetition is 6 bytes
//         magic_packet[offset..offset + 6].copy_from_slice(mac_addr.as_bytes());
//     }
//     println!("Magic packet:");
//     for chunk in magic_packet.chunks(6) {
//         println!("{:x?}", chunk);
//     }
//
//     let mut sock = UdpSocket::bind("0.0.0.0:0".parse::<SocketAddr>().unwrap())
//         .await
//         .unwrap();
//     sock.set_broadcast(true).unwrap();
//     sock.send_to(
//         &magic_packet,
//         "255.255.255.255:9".parse::<SocketAddr>().unwrap(),
//     )
//     .await
//     .unwrap();
//
//     println!("Sent magic packet!");
// }
