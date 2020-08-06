use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use pnet::datalink::MacAddr;
use serde::Deserialize;
use tokio::fs;

use crate::id::DeviceId;

#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Configuration {
    devices: HashMap<String, DeviceConfiguration>,

    tftp_directory: PathBuf,

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

    /// Directory to place GRUB config files to serve over TFTP in, for example `/srv/tftp`.
    pub fn tftp_directory(&self) -> &Path {
        self.tftp_directory.as_path()
    }
}

/// Configuration for an individual device
#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct DeviceConfiguration {
    agent: String,

    interface: Option<String>,

    /// MAC address of the device
    mac_address: MacAddr,

    grub_config: PathBuf,

    targets: HashMap<String, TargetConfiguration>
}

impl DeviceConfiguration {
    /// URI of the agent service running on the device
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// Name of the network interface this device can be reached on. If not specified, uses the
    /// configured default interface.
    pub fn interface(&self) -> Option<&str> {
        self.interface.as_deref()
    }

    /// MAC address of this device
    pub fn mac_address(&self) -> MacAddr {
        self.mac_address
    }

    /// Path to the GRUB configuration file for this device, relative to `tftp_directory`. The device must be configured
    /// to download and `source` this file on startup.
    pub fn grub_config(&self) -> &Path {
        &self.grub_config
    }

    pub fn targets(&self) -> &HashMap<String, TargetConfiguration> {
        &self.targets
    }
}

#[derive(Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct TargetConfiguration {
    menu_entry: String,
}

impl TargetConfiguration {
    /// The GRUB menu entry that boots this target.
    pub fn menu_entry(&self) -> &str {
        &self.menu_entry
    }
}