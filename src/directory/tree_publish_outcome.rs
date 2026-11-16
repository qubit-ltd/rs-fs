use crate::metadata::{AchievedAtomicity, PublicationMethod};

/// Facts reported after a staged directory tree is published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreePublishOutcome {
    /// Number of entries reported as published.
    pub entries_published: u64,
    /// Aggregate file bytes reported as published.
    pub content_bytes: u64,
    /// Atomicity actually achieved by publication.
    pub atomicity: AchievedAtomicity,
    /// Publication method used by the provider.
    pub method: PublicationMethod,
    /// Whether the provider confirmed durable publication.
    pub durable: bool,
}
