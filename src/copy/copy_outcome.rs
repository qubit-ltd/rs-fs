// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation outcome.

use crate::copy::CopyConflictPolicy;
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

    /// Replaces provider-native diagnostics that have already passed key
    /// validation.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }

    /// Returns the completed copy statistics.
    #[inline]
    #[must_use]
    pub const fn stats(&self) -> &CopyStats {
        &self.stats
    }
    /// Returns the actual method used by the completed operation.
    #[inline]
    #[must_use]
    pub const fn method(&self) -> CopyMethod {
        self.method
    }
    /// Returns the atomicity actually achieved while publishing the target.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }
    /// Returns whether provider-confirmed durability synchronization completed.
    #[inline]
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }
    /// Replaces the provider-reported durability completion fact.
    #[inline]
    #[must_use]
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Records the metadata preservation policy actually achieved by the
    /// provider.
    #[inline]
    #[must_use]
    pub fn with_metadata(mut self, metadata: MetadataPreservePolicy) -> Self {
        self.metadata = metadata;
        self
    }

    /// Records the destination version reported after publication.
    #[inline]
    #[must_use]
    pub fn with_target_version(mut self, target_version: ResourceVersion) -> Self {
        self.target_version = Some(target_version);
        self
    }
    /// Returns the metadata preservation result represented by this outcome.
    #[inline]
    #[must_use]
    pub const fn metadata(&self) -> MetadataPreservePolicy {
        self.metadata
    }
    /// Returns the target version when the provider reported one.
    #[inline]
    #[must_use]
    pub const fn target_version(&self) -> Option<&ResourceVersion> {
        self.target_version.as_ref()
    }
    /// Returns whether the facade streamed after the provider declined its fast
    /// path.
    #[inline]
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
        self.used_fallback
    }
    /// Returns provider diagnostics that are safe to expose.
    #[inline]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }
    /// Marks this result as the facade's streamed fallback.
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

    /// Returns the first provider-completed outcome fact that contradicts the
    /// resolved copy request.
    pub(crate) fn contract_violation(&self, options: &CopyOptions) -> Option<&'static str> {
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
        let entries = self
            .stats
            .files
            .checked_add(self.stats.directories)
            .and_then(|value| value.checked_add(self.stats.symlinks))
            .and_then(|value| value.checked_add(self.stats.objects))
            .and_then(|value| value.checked_add(self.stats.prefixes));
        let valid_skipped_file = options.mode() == CopyMode::File
            && options.conflict() == CopyConflictPolicy::Skip
            && self.stats.skipped == 1
            && entries == Some(0);
        let valid_copied_file = entries == Some(1) && self.stats.directories == 0 && self.stats.prefixes == 0;
        if options.mode() == CopyMode::File && !valid_copied_file && !valid_skipped_file {
            return Some("provider reported a file-mode copy without exactly one resource");
        }
        if entries.is_none()
            || options.max_entries().is_some_and(|maximum| {
                u64::try_from(maximum).is_ok_and(|maximum| entries.is_some_and(|entries| entries > maximum))
            })
        {
            return Some("provider reported copy entries beyond the requested limit");
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::CopyOutcome;
    use crate::copy::CopyMethod;
    use crate::copy::CopyOptions;
    use crate::copy::CopyStats;
    use crate::copy::MetadataPreservePolicy;
    use crate::metadata::AchievedAtomicity;
    use crate::metadata::NonSensitiveMetadata;
    use crate::metadata::ResourceVersion;

    #[test]
    fn outcome_accessors_are_executed_at_runtime() {
        let constructor: fn(CopyStats, CopyMethod, AchievedAtomicity) -> CopyOutcome = black_box(CopyOutcome::new);
        let metadata: fn(&CopyOutcome) -> MetadataPreservePolicy = black_box(CopyOutcome::metadata);
        let target_version: for<'a> fn(&'a CopyOutcome) -> Option<&'a ResourceVersion> =
            black_box(CopyOutcome::target_version);
        let diagnostics: fn(&CopyOutcome) -> &NonSensitiveMetadata = black_box(CopyOutcome::diagnostics);
        let with_target_version: fn(CopyOutcome, ResourceVersion) -> CopyOutcome =
            black_box(CopyOutcome::with_target_version);
        let with_diagnostics: fn(CopyOutcome, crate::metadata::UserMetadata) -> CopyOutcome =
            black_box(CopyOutcome::with_diagnostics);

        let outcome = with_diagnostics(
            with_target_version(
                constructor(CopyStats::default(), CopyMethod::Native, AchievedAtomicity::Atomic),
                ResourceVersion::new("generation-7"),
            ),
            crate::metadata::UserMetadata::new(),
        );
        assert_eq!(MetadataPreservePolicy::None, metadata(&outcome));
        assert_eq!(
            Some("generation-7"),
            target_version(&outcome).map(ResourceVersion::as_str)
        );
        assert!(diagnostics(&outcome).is_empty());

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
}
