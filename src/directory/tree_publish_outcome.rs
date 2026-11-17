// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::metadata::DurabilityRequirement;
use crate::metadata::PublicationMethod;
use crate::metadata::TreePublishOptions;

/// Facts reported after a staged directory tree is published.
///
/// # Examples
///
/// ```
/// use qubit_fs::directory::TreePublishOutcome;
/// use qubit_fs::metadata::AchievedAtomicity;
/// use qubit_fs::metadata::PublicationMethod;
///
/// let outcome = TreePublishOutcome {
///     entries_published: 3,
///     content_bytes: 4096,
///     atomicity: AchievedAtomicity::Atomic,
///     method: PublicationMethod::AtomicRename,
///     durable: true,
/// };
/// assert!(outcome.facts_are_consistent());
/// ```
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

impl TreePublishOutcome {
    /// Reports whether publication facts are consistent with the method used.
    ///
    /// Per-object and streamed publication methods do not establish one
    /// atomic namespace switch for a directory tree. This method validates
    /// provider facts without rewriting or discarding them.
    ///
    /// # Returns
    /// `true` when the recorded atomicity is consistent with the publication
    /// method, or when the method does not impose a stricter consistency rule.
    #[must_use]
    #[inline]
    pub const fn facts_are_consistent(&self) -> bool {
        match self.method {
            PublicationMethod::AtomicRename => {
                matches!(self.atomicity, AchievedAtomicity::Atomic)
            }
            PublicationMethod::ConditionalReplace => true,
            PublicationMethod::Direct
            | PublicationMethod::ServerSideCopy
            | PublicationMethod::StreamCopy
            | PublicationMethod::CopyThenDelete => !matches!(self.atomicity, AchievedAtomicity::Atomic),
        }
    }

    /// Reports whether the actual facts satisfy the requested guarantees.
    ///
    /// The outcome remains intact when this returns `false`, so callers can
    /// still inspect partial counts and the guarantees actually achieved.
    ///
    /// # Parameters
    /// - `options`: Atomicity and durability requirements requested for publication.
    ///
    /// # Returns
    /// `true` when the recorded facts satisfy the requested guarantees and are
    /// internally consistent.
    #[must_use]
    #[inline]
    pub const fn meets_requirements(&self, options: &TreePublishOptions) -> bool {
        let atomicity_met = match options.atomicity {
            AtomicityRequirement::Required => {
                matches!(self.atomicity, AchievedAtomicity::Atomic)
            }
            AtomicityRequirement::Preferred | AtomicityRequirement::NotRequired => true,
        };
        let durability_met = match options.durability {
            DurabilityRequirement::Required => self.durable,
            DurabilityRequirement::Preferred | DurabilityRequirement::NotRequired => true,
        };
        atomicity_met && durability_met && self.facts_are_consistent()
    }
}
