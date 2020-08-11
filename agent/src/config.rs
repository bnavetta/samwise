use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfiguration {
    pub listen_address: String,
    pub target_name: String,

    #[serde(default = "default_reboot_command")]
    pub reboot_command: Option<Vec<String>>,

    #[serde(default = "default_shutdown_command")]
    pub shutdown_command: Option<Vec<String>>,

    #[serde(default = "default_suspend_command")]
    pub suspend_command: Option<Vec<String>>,
}

// Note: using AppleScript on macOS because it's supposedly more like a GUI shutdown

/// System-specific default for shutting down
/// - On Linux, use `systemctl`
/// - On macOS, use AppleScript "System Events" actions
/// - on Windows, use `shutdown`
fn default_reboot_command() -> Option<Vec<String>> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            Some(vec!["systemctl".to_string(), "reboot".to_string()])
        } else if #[cfg(target_os = "macos")] {
            Some(vec!["osascript".to_string(), "-e".to_string(), "tell app \"System Events\" to restart".to_string()])
        } else if #[cfg(target_os = "windows")] {
            Some(vec!["shutdown".to_string(), "/r".to_string()])
        } else {
            None
        }
    }
}

/// System-specific default for shutting down
/// - On Linux, use `systemctl`
/// - On macOS, use AppleScript "System Events" actions
/// - on Windows, use `shutdown`
fn default_shutdown_command() -> Option<Vec<String>> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            Some(vec!["systemctl".to_string(), "poweroff".to_string()])
        } else if #[cfg(target_os = "macos")] {
            Some(vec!["osascript".to_string(), "-e".to_string(), "tell app \"System Events\" to shut down".to_string()])
        } else if #[cfg(target_os = "windows")] {
            Some(vec!["shutdown".to_string(), "/s".to_string()])
        } else {
            None
        }
    }
}

/// System-specific default for suspending.
/// - On Linux, use `systemctl`
/// - On macOS, uses `pmset`
/// - On Windows, no default because it depends on whether or not hibernation is enabled and if a third-party helper is available
fn default_suspend_command() -> Option<Vec<String>> {
    cfg_if::cfg_if! {
        if #[cfg(target_os = "linux")] {
            Some(vec!["systemctl".to_string(), "suspend".to_string()])
        } else if #[cfg(target_os = "macos")] {
            Some(vec!["pmset".to_string(), "sleepnow".to_string()])
        } else {
            None
        }
    }
}
