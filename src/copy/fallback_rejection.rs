// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Reasons the allowlisted stream copy cannot serve options.

/// First static reason a stream fallback cannot honor a copy request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum FallbackRejection {
    /// Tree traversal was requested.
    TreeMode,
    /// The symlink override differs from the filesystem policy.
    SymlinkPolicyOverride,
    /// Continue-on-error tree behavior was requested.
    ContinueOnError,
    /// Metadata preservation was requested.
    MetadataPreservation,
    /// Server-side copy was required.
    ServerSideRequired,
    /// Missing destination parents were requested.
    CreateParent,
    /// Durable publication was required.
    DurabilityRequired,
    /// Atomic skip cannot be represented by create-new fallback.
    AtomicSkip,
    /// The conflict policy is outside the fallback allowlist.
    ConflictPolicy,
    /// The provider lacks read support.
    MissingRead,
    /// The provider lacks write support.
    MissingWrite,
    /// The provider lacks stat support.
    MissingStat,
}
