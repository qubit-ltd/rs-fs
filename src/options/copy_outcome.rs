// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation outcome.

use crate::{
    AchievedAtomicity, CopyConflictPolicy, CopyMethod, CopyOptions, CopyStats,
    MetadataPreservePolicy, NonSensitiveMetadata, ResourceVersion, ServerSidePreference,
    UserMetadata,
};

/// Outcome returned by copy operations.
#[derive(Clone, Debug, PartialEq)]
pub struct CopyOutcome {
    stats: CopyStats,
    method: CopyMethod,
    atomicity: AchievedAtomicity,
    durable: bool,
    metadata: MetadataPreservePolicy,
    target_version: Option<ResourceVersion>,
    used_fallback: bool,
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
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }

    /// Returns the completed copy statistics.
    #[must_use]
    pub const fn stats(&self) -> &CopyStats {
        &self.stats
    }
    /// Returns the actual method used by the completed operation.
    #[must_use]
    pub const fn method(&self) -> CopyMethod {
        self.method
    }
    /// Returns the atomicity actually achieved while publishing the target.
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }
    /// Returns whether provider-confirmed durability synchronization completed.
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }
    /// Replaces the provider-reported durability completion fact.
    #[must_use]
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Records the metadata preservation policy actually achieved by the
    /// provider.
    #[must_use]
    pub fn with_metadata(mut self, metadata: MetadataPreservePolicy) -> Self {
        self.metadata = metadata;
        self
    }

    /// Records the destination version reported after publication.
    #[must_use]
    pub fn with_target_version(mut self, target_version: ResourceVersion) -> Self {
        self.target_version = Some(target_version);
        self
    }
    /// Returns the metadata preservation result represented by this outcome.
    #[must_use]
    pub const fn metadata(&self) -> MetadataPreservePolicy {
        self.metadata
    }
    /// Returns the target version when the provider reported one.
    #[must_use]
    pub const fn target_version(&self) -> Option<&ResourceVersion> {
        self.target_version.as_ref()
    }
    /// Returns whether the facade streamed after the provider declined its fast
    /// path.
    #[must_use]
    pub const fn used_fallback(&self) -> bool {
        self.used_fallback
    }
    /// Returns provider diagnostics that are safe to expose.
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }
    /// Marks this result as the facade's streamed fallback.
    pub(crate) fn streamed_fallback(stats: CopyStats, atomicity: AchievedAtomicity) -> Self {
        Self {
            stats,
            method: CopyMethod::Streamed,
            atomicity,
            durable: false,
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
        if options.server_side == ServerSidePreference::Require
            && self.method != CopyMethod::ServerSide
        {
            return Some(
                "provider reported a non-server-side success for a server-side-required copy",
            );
        }
        if self.metadata != options.preserve_metadata {
            return Some("provider reported metadata preservation different from the copy request");
        }
        if !options.continue_on_error && self.stats.failed != 0 {
            return Some("provider reported failed copy entries without continue-on-error");
        }
        if options.conflict != CopyConflictPolicy::Skip && self.stats.skipped != 0 {
            return Some("provider reported skipped copy entries without a skip conflict policy");
        }
        if options.conflict != CopyConflictPolicy::Overwrite && self.stats.overwritten != 0 {
            return Some(
                "provider reported overwritten copy entries without an overwrite conflict policy",
            );
        }
        None
    }
}
