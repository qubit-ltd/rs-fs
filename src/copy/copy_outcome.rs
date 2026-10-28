// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation outcome.

use crate::copy::CopyConflictPolicy;
use crate::copy::CopyFailureState;
use crate::copy::CopyMethod;
use crate::copy::CopyMode;
use crate::copy::CopyOptions;
use crate::copy::CopyStats;
use crate::copy::MetadataPreservePolicy;
use crate::copy::ServerSidePreference;
use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::ResourceVersion;
use crate::metadata::UserMetadata;

/// Outcome returned by copy operations.
///
/// Facades validate the reported statistics shape before checking requested
/// guarantees. When a guarantee is violated, coherent statistics still
/// determine whether the destination is unchanged, published, or partial.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::{CopyMethod, CopyOutcome, CopyStats};
/// use qubit_fs::metadata::AchievedAtomicity;
///
/// let outcome = CopyOutcome::new(CopyStats::default(), CopyMethod::Streamed, AchievedAtomicity::Atomic);
/// assert_eq!(CopyMethod::Streamed, outcome.method());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOutcome {
    /// Aggregate counts and bytes reported for the completed copy.
    stats: CopyStats,
    /// Actual transfer method used by the provider or facade.
    method: CopyMethod,
    /// Atomicity achieved while publishing the destination.
    atomicity: AchievedAtomicity,
    /// Whether requested durability was confirmed.
    durable: bool,
    /// Metadata preservation level actually achieved.
    metadata: MetadataPreservePolicy,
    /// Optional version assigned to the destination.
    target_version: Option<ResourceVersion>,
    /// Whether the facade completed the copy after provider decline.
    used_fallback: bool,
    /// Scrubbed provider diagnostics.
    diagnostics: NonSensitiveMetadata,
}

impl CopyOutcome {
    /// Creates a copy outcome.
    ///
    /// # Parameters
    /// - `stats`: Copy statistics.
    /// - `method`: Method used to complete the copy.
    /// - `atomicity`: Atomicity achieved while publishing the destination.
    ///
    /// # Returns
    /// New copy outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(stats: CopyStats, method: CopyMethod, atomicity: AchievedAtomicity) -> Self {
        Self {
            stats,
            method,
            atomicity,
            durable: false,
            metadata: MetadataPreservePolicy::None,
            target_version: None,
            used_fallback: false,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Marks this result as the facade's streamed fallback.
    ///
    /// # Parameters
    /// - `stats`: Counts and bytes accumulated by the fallback copy.
    /// - `atomicity`: Atomicity achieved while publishing the destination.
    /// - `durable`: Whether durability synchronization was confirmed.
    ///
    /// # Returns
    /// An outcome marked as fallback with no provider diagnostics or target
    /// version.
    #[must_use]
    pub(crate) fn streamed_fallback(stats: CopyStats, atomicity: AchievedAtomicity, durable: bool) -> Self {
        Self {
            stats,
            method: CopyMethod::Streamed,
            atomicity,
            durable,
            metadata: MetadataPreservePolicy::None,
            target_version: None,
            used_fallback: true,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Returns the completed copy statistics.
    ///
    /// # Returns
    /// A borrow of the aggregate counts and bytes reported by the operation.
    #[inline]
    #[must_use]
    pub const fn stats(&self) -> &CopyStats {
        &self.stats
    }

    /// Returns the actual method used by the completed operation.
    ///
    /// # Returns
    /// The method the provider or facade used to transfer the resources.
    #[inline]
    #[must_use]
    pub const fn method(&self) -> CopyMethod {
        self.method
    }

    /// Returns the atomicity actually achieved while publishing the target.
    ///
    /// # Returns
    /// The achieved atomicity, which may be weaker than the requested level.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }

    /// Returns whether provider-confirmed durability synchronization completed.
    ///
    /// # Returns
    /// `true` only when the provider confirmed the requested synchronization.
    #[inline]
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }

    /// Returns the metadata preservation result represented by this outcome.
    ///
    /// # Returns
    /// The metadata preservation policy the provider reports as achieved.
    #[inline]
    #[must_use]
    pub const fn metadata(&self) -> MetadataPreservePolicy {
        self.metadata
    }

    /// Returns the target version reported by the provider or fallback writer.
    ///
    /// # Returns
    /// `Some` with the destination version when reported, or `None` otherwise.
    #[inline]
    pub const fn target_version(&self) -> Option<&ResourceVersion> {
        self.target_version.as_ref()
    }

    /// Returns whether the facade streamed after the provider declined its fast
    /// path.
    ///
    /// # Returns
    /// `true` when the facade completed this operation using streamed fallback.
    #[inline]
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
        self.used_fallback
    }

    /// Returns provider diagnostics that are safe to expose.
    ///
    /// # Returns
    /// A borrow of validated diagnostics with sensitive keys removed.
    #[inline]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }

    /// Replaces provider-native diagnostics that have already passed key
    /// validation.
    ///
    /// # Parameters
    /// - `diagnostics`: Provider metadata whose keys have passed validation.
    ///
    /// # Returns
    /// This outcome with the validated diagnostics stored for safe exposure.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }

    /// Replaces the provider-reported durability completion fact.
    ///
    /// # Parameters
    /// - `durable`: Whether the provider confirmed durability synchronization.
    ///
    /// # Returns
    /// This outcome with the provider's durability fact recorded.
    #[inline]
    #[must_use]
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Records the metadata preservation policy actually achieved by the
    /// provider.
    ///
    /// # Parameters
    /// - `metadata`: The preservation policy the provider reports as achieved.
    ///
    /// # Returns
    /// This outcome with the achieved metadata policy recorded.
    #[inline]
    #[must_use]
    pub fn with_metadata(mut self, metadata: MetadataPreservePolicy) -> Self {
        self.metadata = metadata;
        self
    }

    /// Records the destination version reported after publication.
    ///
    /// # Parameters
    /// - `target_version`: Version reported for the published destination.
    ///
    /// # Returns
    /// This outcome with the destination version recorded.
    #[inline]
    #[must_use]
    pub fn with_target_version(mut self, target_version: ResourceVersion) -> Self {
        self.target_version = Some(target_version);
        self
    }

    /// Returns the first provider-completed outcome fact that contradicts the
    /// resolved copy request.
    ///
    /// # Parameters
    /// - `options`: The resolved request whose constraints the outcome must
    ///   meet.
    ///
    /// # Returns
    /// The first violated contract, or `None` when all checked constraints
    /// hold.
    pub(crate) fn contract_violation(&self, options: &CopyOptions) -> Option<&'static str> {
        if self.statistics_shape_violation(options.mode()).is_some() {
            return Some("provider reported structurally inconsistent copy statistics");
        }
        if self.used_fallback || self.method == CopyMethod::Streamed {
            return Some("provider returned a facade streamed-fallback outcome as native success");
        }
        if options.atomicity() == crate::metadata::AtomicityRequirement::Required
            && self.atomicity != AchievedAtomicity::Atomic
        {
            return Some("provider reported non-atomic success for an atomic-required copy");
        }
        if options.durability() == crate::metadata::DurabilityRequirement::Required && !self.durable {
            return Some("provider reported non-durable success for a durability-required copy");
        }
        if options.server_side() == ServerSidePreference::Require && self.method != CopyMethod::ServerSide {
            return Some("provider reported a non-server-side success for a server-side-required copy");
        }
        if options.server_side() == ServerSidePreference::Disable && self.method == CopyMethod::ServerSide {
            return Some("provider reported a server-side success for a server-side-disabled copy");
        }
        if self.metadata != options.preserve_metadata() {
            return Some("provider reported metadata preservation different from the copy request");
        }
        if !options.continue_on_error() && self.stats.failed != 0 {
            return Some("provider reported failed copy entries without continue-on-error");
        }
        if options.conflict() != CopyConflictPolicy::Skip && self.stats.skipped != 0 {
            return Some("provider reported skipped copy entries without a skip conflict policy");
        }
        if options.conflict() != CopyConflictPolicy::Overwrite && self.stats.overwritten != 0 {
            return Some("provider reported overwritten copy entries without an overwrite conflict policy");
        }
        if options.max_bytes().is_some_and(|maximum| self.stats.bytes > maximum) {
            return Some("provider reported copy bytes beyond the requested limit");
        }
        let entries = self.entry_count();
        if entries.is_none()
            || options.max_entries().is_some_and(|maximum| {
                u64::try_from(maximum).is_ok_and(|maximum| entries.is_some_and(|entries| entries > maximum))
            })
        {
            return Some("provider reported copy entries beyond the requested limit");
        }
        None
    }

    /// Infers destination publication facts for a rejected completed outcome.
    ///
    /// Statistics must form a coherent shape for the requested mode before
    /// they can establish publication. A request guarantee violation alone
    /// does not invalidate otherwise coherent statistics.
    pub(crate) fn violation_failure_state(&self, options: &CopyOptions) -> CopyFailureState {
        if self.statistics_shape_violation(options.mode()).is_some() {
            return CopyFailureState::Indeterminate;
        }
        crate::copy::internal::from_completed_stats(&self.stats)
    }

    fn entry_count(&self) -> Option<u64> {
        // These counters describe completed resources; skipped and failed
        // entries are separate outcomes and cannot justify an overwrite.
        self.stats
            .files
            .checked_add(self.stats.directories)
            .and_then(|value| value.checked_add(self.stats.symlinks))
            .and_then(|value| value.checked_add(self.stats.objects))
            .and_then(|value| value.checked_add(self.stats.prefixes))
    }

    fn statistics_shape_violation(&self, mode: CopyMode) -> Option<&'static str> {
        let Some(entries) = self.entry_count() else {
            return Some("provider reported copy entry counters that overflow");
        };
        if self.stats.overwritten > entries {
            return Some("provider reported more overwritten entries than completed resources");
        }
        if mode != CopyMode::File {
            return None;
        }
        let valid_skipped_file = self.stats.skipped == 1
            && entries == 0
            && self.stats.bytes == 0
            && self.stats.overwritten == 0
            && self.stats.failed == 0;
        let valid_copied_file = entries == 1
            && self.stats.directories == 0
            && self.stats.prefixes == 0
            && self.stats.skipped == 0
            && self.stats.failed == 0;
        if valid_copied_file || valid_skipped_file {
            None
        } else {
            Some("provider reported a file-mode copy without exactly one resource")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CopyOutcome;
    use crate::copy::CopyFailureState;
    use crate::copy::CopyMethod;
    use crate::copy::CopyOptions;
    use crate::copy::CopyStats;
    use crate::metadata::AchievedAtomicity;
    use crate::metadata::AtomicityRequirement;

    #[test]
    fn test_contract_violation_rejects_exceeded_copy_limits() {
        let bytes_exceeded = CopyOutcome::new(
            CopyStats {
                bytes: 11,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert!(
            bytes_exceeded
                .contract_violation(&CopyOptions::default().with_max_bytes(Some(10)))
                .is_some()
        );

        let entries_exceeded = CopyOutcome::new(
            CopyStats {
                files: 2,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert!(
            entries_exceeded
                .contract_violation(&CopyOptions::tree().with_max_entries(Some(1)))
                .is_some()
        );
    }

    #[test]
    fn test_violation_failure_state_uses_coherent_statistics() {
        let skipped = CopyOutcome::new(
            CopyStats {
                skipped: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::NonAtomic,
        );
        let file_skip = CopyOptions::file()
            .with_conflict(crate::copy::CopyConflictPolicy::Skip)
            .with_atomicity(AtomicityRequirement::Required);
        assert_eq!(
            Some("provider reported non-atomic success for an atomic-required copy"),
            skipped.contract_violation(&file_skip)
        );
        assert_eq!(CopyFailureState::Unchanged, skipped.violation_failure_state(&file_skip));

        let copied_empty_file = CopyOutcome::new(
            CopyStats {
                files: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::NonAtomic,
        );
        assert_eq!(
            CopyFailureState::Published,
            copied_empty_file.violation_failure_state(&file_skip)
        );

        let failed_only = CopyOutcome::new(
            CopyStats {
                failed: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert_eq!(
            CopyFailureState::Indeterminate,
            failed_only.violation_failure_state(&CopyOptions::default())
        );

        let partial_tree = CopyOutcome::new(
            CopyStats {
                files: 1,
                failed: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::NonAtomic,
        );
        assert_eq!(
            CopyFailureState::PartiallyPublished,
            partial_tree.violation_failure_state(&CopyOptions::tree().with_atomicity(AtomicityRequirement::Required)),
        );

        let inconsistent = CopyOutcome::new(
            CopyStats {
                files: 1,
                skipped: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert_eq!(
            CopyFailureState::Indeterminate,
            inconsistent
                .violation_failure_state(&CopyOptions::file().with_conflict(crate::copy::CopyConflictPolicy::Skip)),
        );

        let too_many_overwrites = CopyOutcome::new(
            CopyStats {
                files: 1,
                overwritten: 2,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert!(too_many_overwrites.contract_violation(&CopyOptions::file()).is_some());
        assert_eq!(
            CopyFailureState::Indeterminate,
            too_many_overwrites.violation_failure_state(&CopyOptions::file())
        );

        let overflow = CopyOutcome::new(
            CopyStats {
                files: u64::MAX,
                objects: 1,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            AchievedAtomicity::Atomic,
        );
        assert_eq!(
            CopyFailureState::Indeterminate,
            overflow.violation_failure_state(&CopyOptions::default())
        );
    }

    #[test]
    fn test_all_modes_reject_overwritten_counts_above_completed_entries() {
        let outcome = CopyOutcome::new(
            CopyStats {
                files: 1,
                overwritten: 2,
                ..CopyStats::default()
            },
            CopyMethod::Native,
            crate::metadata::AchievedAtomicity::Atomic,
        );

        for options in [CopyOptions::default(), CopyOptions::tree()] {
            assert!(outcome.contract_violation(&options).is_some());
            assert_eq!(
                CopyFailureState::Indeterminate,
                outcome.violation_failure_state(&options)
            );
        }
    }
}
