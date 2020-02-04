use serde::Deserialize;
use serde::de::Deserializer;

use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{PathBuf, Path};

use anyhow::{Error, Context, bail};
use eui48::MacAddress;
use toml;

// Config is immutable, parsed on startup, shared with all tasks
// Long-running tasks:
// - HTTP API server
//    - sends messages to other tasks based on API calls
//    - also passes through status requests to device (check if up/down and which OS it's running)
// - DHCP updater
//     - receives BootTargetChange messages on a MPSC queue
//     - on change, regenerates DHCP server config and restarts server
//     - maybe include a oneshot queue in BootTargetChange, so we can ensure DHCP server updates before booting device (assuming systemctl blocks long enough)
//     - note: use reload-or-restart instead of restart to reload DHCP server if supported
//     - fancy: debounce updates to minimize DHCP server churn (tokio config-watching example maybe?)
// - Device state updater
//     - receives DeviceStateChange messages on a MPSC queue
//     - on change, sends WOL packet or shutdown RPC
// - maybe use status RPC to update internal state (which boot target) after service restart (maybe generalize this)
//     - or save on disk (could use a broadcast queue to update this + DHCP task)

#[derive(Deserialize, Eq, PartialEq, Hash, Debug)]
#[serde(try_from = "String")]
pub struct DeviceId(String);

impl TryFrom<String> for DeviceId {
    type Error = Error;

    fn try_from(value: String) -> Result<DeviceId, Error> {
        if value.chars().all(char::is_alphanumeric) {
            Ok(DeviceId(value))
        } else {
            bail!("Device IDs must contain only alphanumeric characters. Got \"{}\"", value)
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct DeviceConfig {
    /// MAC address of the target device
    #[serde(deserialize_with = "deserialize_mac_address")]
    mac_address: MacAddress,

    /// Hostname or address for issuing RPCs to the device
    host: String,

    /// Possible OS targets for the device to boot. Keys are the name to use for the boot target and values are
    /// GRUB menu entries (either numbers or names)
    // TODO: will need to configure agent with boot target to report back
    boot_targets: HashMap<String, String>
}

impl DeviceConfig {
    pub fn mac_address(&self) -> MacAddress {
        self.mac_address
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn boot_targets(&self) -> &HashMap<String, String> {
        &self.boot_targets
    }
}

/// A DHCP server program
#[derive(Deserialize, Copy, Clone, Eq, PartialEq, Debug)]
pub enum DhcpServer {
    Dnsmasq,
    IscDhcpd
}

/// Configures how to update the DHCP server
#[derive(Deserialize, Debug)]
pub struct DhcpConfig {
    /// Which DHCP server to use
    server: DhcpServer,

    /// Path to a configuration file that Samwise can update which will be loaded by the DHCP server
    config_path: PathBuf,
}

impl DhcpConfig {
    pub fn server(&self) -> DhcpServer {
        self.server
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    devices: HashMap<DeviceId, DeviceConfig>,
    dhcp: DhcpConfig,
}

impl Config {
    pub fn load(source: &str) -> Result<Config, Error> {
        Ok(toml::from_str(source).context("Could not parse configuration")?)
    }

    pub fn devices(&self) -> &HashMap<DeviceId, DeviceConfig> {
        &self.devices
    }

    pub fn dhcp(&self) -> &DhcpConfig {
        &self.dhcp
    }
}

fn deserialize_mac_address<'de, D>(deserializer: D) -> Result<MacAddress, D::Error> where D: Deserializer<'de> {
    let s = String::deserialize(deserializer)?;
    MacAddress::parse_str(&s).map_err(serde::de::Error::custom)
}