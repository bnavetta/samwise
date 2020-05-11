//! Data model for Samwise

use std::fmt;

/// Identifier referring to a particular device. For example, `htpc` or `my-desktop`.
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new<S: Into<String>>(s: S) -> DeviceId {
        DeviceId(s.into())
    }

    pub fn as_string(&self) -> &String {
        &self.0
    }
}

impl slog::Value for DeviceId {
    fn serialize(
        &self,
        record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result<()> {
        serializer.emit_str(key, &self.0)
    }
}

/// Identifier for a bootable target. For example, `windows` or `ubuntu-lts`.
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetId(String);

impl TargetId {
    pub fn new<S: Into<String>>(s: S) -> TargetId {
        TargetId(s.into())
    }

    pub fn as_string(&self) -> &String {
        &self.0
    }
}

impl fmt::Display for TargetId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_string())
    }
}

/// Current state of a device
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DeviceState {
    /// The device is in an unknown state.
    Unknown,

    /// The device is starting up. Generally, Samwise only knows if a device is `Starting` if it
    /// requested it.
    Starting { target: TargetId },

    /// The device is fully up-and-running.
    Running {
        /// If known, the currently-running target.
        target: Option<TargetId>,
    },

    /// The device is shutting down (or suspending). Like `Starting`, Samwise does not always know
    /// if a device is shutting down.
    ShuttingDown,

    /// The device is off (or suspended).
    ShutDown,
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &DeviceState::Unknown => write!(f, "Unknown"),
            &DeviceState::Starting { ref target } => write!(f, "Starting {}", target),
            &DeviceState::Running {
                target: Some(ref target),
            } => write!(f, "Running {}", target),
            &DeviceState::Running { target: None } => write!(f, "Running unknown OS"),
            &DeviceState::ShuttingDown => write!(f, "Shutting down"),
            &DeviceState::ShutDown => write!(f, "Shut down"),
        }
    }
}

/// Desired state for a device
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DesiredState {
    Running(TargetId),
    ShutDown,
}
