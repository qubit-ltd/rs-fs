/// Point-in-time storage capacity observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum StorageObservation {
    /// Available and total capacity are known in bytes.
    Known {
        /// Available capacity.
        available_bytes: u64,
        /// Total capacity.
        total_bytes: u64,
    },
    /// The provider cannot report capacity.
    Unknown,
}
