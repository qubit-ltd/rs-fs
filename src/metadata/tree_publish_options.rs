use super::{AtomicityRequirement, DurabilityRequirement};
/// Publication guarantees requested for a staged directory tree.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreePublishOptions {
    /// Required namespace publication atomicity.
    pub atomicity: AtomicityRequirement,
    /// Required persistence strength after publication.
    pub durability: DurabilityRequirement,
}
