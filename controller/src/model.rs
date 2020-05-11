//! Data model for Samwise

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
    fn serialize(&self, record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result<()> {
        serializer.emit_str(key, &self.0)
    }
}

/// Identifier for a bootable target. For example, `windows` or `ubuntu-lts`.
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TargetId(String);

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
        target: Option<TargetId>
    },

    /// The device is shutting down (or suspending). Like `Starting`, Samwise does not always know
    /// if a device is shutting down.
    ShuttingDown,

    /// The device is off (or suspended).
    Off
}