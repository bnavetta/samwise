//! Typed identifiers

use std::fmt;

/// Identifier referring to a particular device. For example, `htpc` or `my-desktop`.
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DeviceId(String);

impl DeviceId {
    pub fn new<S: Into<String>>(s: S) -> DeviceId {
        DeviceId(s.into())
    }

    pub fn as_string(&self) -> &String {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_string())
    }
}

impl slog::Value for DeviceId {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result<()> {
        serializer.emit_str(key, &self.0)
    }
}

/// Identifier for a bootable target. For example, `windows` or `ubuntu-lts`.
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
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
