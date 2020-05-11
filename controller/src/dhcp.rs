use std::collections::HashMap;
use std::fmt::{self, Write};

use anyhow::{Context, Result};
use tokio::fs;
use tokio::prelude::*;
use tokio::process::Command;

use crate::device::Device;

/// A supported DHCP server that Samwise can use to pass options to a target computer
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DhcpServerKind {
    Dnsmasq,
    IscDhcpd,
}

impl fmt::Display for DhcpServerKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DhcpServerKind::Dnsmasq => write!(f, "dnsmasq"),
            DhcpServerKind::IscDhcpd => write!(f, "ISC DHCPD"),
        }
    }
}

impl DhcpServerKind {
    /// The systemd service name this DHCP server runs as
    pub fn service_name(&self) -> &str {
        match *self {
            DhcpServerKind::Dnsmasq => "dnsmasq",
            DhcpServerKind::IscDhcpd => "isc-dhcp-server",
        }
    }

    pub fn generate_config(&self, config: &HashMap<&Device, String>) -> String {
        match *self {
            DhcpServerKind::Dnsmasq => {
                let mut buf = String::new();
                for (device, boot_info) in config.iter() {
                    writeln!(
                        buf,
                        "dhcp-host={},set:{}",
                        device.mac_address().to_hex_string(),
                        device.id()
                    )
                    .unwrap();
                    writeln!(
                        buf,
                        "dhcp-option-force=tag:{},209,\"{}\"",
                        device.id(),
                        boot_info
                    )
                    .unwrap();
                }
                buf
            }
            DhcpServerKind::IscDhcpd => {
                let mut buf = String::new();
                for (device, boot_info) in config.iter() {
                    writeln!(buf, "class \"{}\" {{", device.id()).unwrap();
                    writeln!(
                        buf,
                        "    match if hardware = 1:{};",
                        device.mac_address().to_hex_string()
                    )
                    .unwrap();
                    writeln!(buf, "    option loader-configfile \"{}\"", boot_info).unwrap();
                    writeln!(buf, "}}").unwrap();
                }
                buf
            }
        }
    }
}

pub struct DhcpServer {
    kind: DhcpServerKind,
    config_path: String,
}

pub async fn reconfigure(server: &DhcpServer, config: &HashMap<&Device, String>) -> Result<()> {
    let mut file = fs::File::create(&server.config_path)
        .await
        .with_context(|| {
            format!(
                "Could not create DHCP configuration file {}",
                server.config_path
            )
        })?;

    let rendered_config = server.kind.generate_config(config);
    // TODO: log

    file.write_all(rendered_config.as_bytes())
        .await
        .context("Could not write DHCP configuration")?;

    let reload_process = Command::new("systemctl")
        .arg("reload")
        .arg(server.kind.service_name())
        .spawn()
        .with_context(|| format!("Could not reload {}", server.kind))?;

    reload_process
        .await
        .with_context(|| format!("Reloading {} failed", server.kind))?;

    Ok(())
}
