use super::FileSystemCapabilities;

/// Per-mode capability snapshot for a configured filesystem provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FileSystemCapabilityProfile {
    /// Capability strengths for synchronous provider paths.
    pub synchronous: FileSystemCapabilities,
    /// Capability strengths for asynchronous provider paths.
    pub asynchronous: FileSystemCapabilities,
}
impl FileSystemCapabilityProfile {
    /// Creates a profile with independently supplied sync and async snapshots.
    pub const fn new(
        synchronous: FileSystemCapabilities,
        asynchronous: FileSystemCapabilities,
    ) -> Self {
        Self {
            synchronous,
            asynchronous,
        }
    }
}
