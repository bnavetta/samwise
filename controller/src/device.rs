use std::ffi::OsStr;
use std::future::Future;

use anyhow::{Context, Result};
use eui48::MacAddress;
use tokio::process::Command;




/// Send a Wake-on-LAN magic packet to the given MAC address.
/// 
/// To send the magic packet, this function calls `etherwake`.
/// 
/// # Arguments
/// `mac_address`
///     the MAC address of the target computer
/// 
/// # Errors
/// * If `etherwake` is not in the `PATH`
/// * If the MAC address is invalid
/// * If `etherwake` cannot send the magic packet
async fn send_wol_packet(mac_address: &MacAddress) -> Result<()> {
    // TODO: reimplement in Rust instead of shelling out to etherwake?
    let process = Command::new("etherwake")
        .arg(mac_address.to_canonical())
        .spawn()
        .context("Could not start etherwake")?;

    process.await.context("Etherwake failed")?;

    Ok(())
}

/// Representation of a Samwise-controlled computer
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Device {
    id: String, // TODO: enforce alphanumeric
    mac_address: MacAddress,
}

impl Device {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn mac_address(&self) -> &MacAddress {
        &self.mac_address
    }
}

/// Boots up the device if it's not already running.
pub fn wake<'a>(device: &'a Device) -> impl Future<Output = Result<()>> + 'a {
    send_wol_packet(&device.mac_address)

    // TODO: block until up? Should probably be a separate function, may not be necessary
}