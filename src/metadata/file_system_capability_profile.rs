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
    /// Derives independent conditional capability snapshots from sync and
    /// async provider entry points.
    ///
    /// Operation presence indicates that the operation can be attempted. It
    /// does not imply a guarantee for every request.
    #[must_use]
    pub fn from_operations(
        synchronous: crate::spi::ProviderOperations,
        asynchronous: crate::spi::ProviderOperations,
    ) -> Self {
        Self::new(
            synchronous.conditional_capabilities(),
            asynchronous.conditional_capabilities(),
        )
    }

    /// Creates a profile with independently supplied sync and async snapshots.
    pub const fn new(synchronous: FileSystemCapabilities, asynchronous: FileSystemCapabilities) -> Self {
        Self {
            synchronous,
            asynchronous,
        }
    }
}
