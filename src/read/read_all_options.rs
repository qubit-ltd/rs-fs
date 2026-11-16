//! Bounds and consistency requirements for complete-file reads.
use super::ReadConsistencyPolicy;
/// Options for a bounded complete-file read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadAllOptions {
    /// Maximum returned bytes; defaults to 16 MiB.
    pub max_bytes: usize,
    /// Consistency requirement for the opened object.
    pub consistency: ReadConsistencyPolicy,
}
impl ReadAllOptions {
    /// Creates explicit read bounds and consistency policy.
    pub const fn new(max_bytes: usize, consistency: ReadConsistencyPolicy) -> Self {
        Self {
            max_bytes,
            consistency,
        }
    }
}
impl Default for ReadAllOptions {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            consistency: ReadConsistencyPolicy::BestEffort,
        }
    }
}
