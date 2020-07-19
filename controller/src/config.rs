use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use pnet::datalink::MacAddr;
use serde::Deserialize;
use tokio::fs;
use toml;

use crate::id::DeviceId;

#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Configuration {
    devices: HashMap<String, DeviceConfiguration>,

    default_interface: String,
}

impl Configuration {
    pub async fn load_file<P: AsRef<Path>>(source_path: P) -> Result<Configuration> {
        let path = source_path.as_ref();
        let source = fs::read_to_string(path)
            .await
            .with_context(|| format!("Could not read configuration file {}", path.display()))?;
        Configuration::load(&source)
            .with_context(|| format!("Invalid configuration file: {}", path.display()))
    }

    pub fn load(source: &str) -> Result<Configuration> {
        let config = toml::from_str(&source)?;
        Ok(config)
    }

    pub fn devices<'a>(&'a self) -> impl Iterator<Item = DeviceId> + 'a {
        self.devices.keys().map(DeviceId::new)
    }

    pub fn device_configs(&self) -> impl Iterator<Item = (DeviceId, &DeviceConfiguration)> {
        self.devices
            .iter()
            .map(|(id, config)| (DeviceId::new(id), config))
    }

    pub fn device_config(&self, id: &DeviceId) -> Option<&DeviceConfiguration> {
        self.devices.get(id.as_string())
    }

    /// Name of the default network interface to use for devices that do not specify one.
    pub fn default_interface(&self) -> &str {
        &self.default_interface
    }
}

/// Configuration for an individual device
#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct DeviceConfiguration {
    agent: String,

    interface: Option<String>,

    /// MAC address of the device
    mac_address: MacAddr,
}

impl DeviceConfiguration {
    /// URI of the agent service running on the device
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// Name of the network interface this device can be reached on. If not specified, uses the
    /// configured default interface.
    pub fn interface(&self) -> Option<&str> {
        self.interface.as_ref().map(|s| s.as_str())
    }

    /// MAC address of this device
    pub fn mac_address(&self) -> MacAddr {
        self.mac_address
    }
}
