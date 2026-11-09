// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared helpers for stream-copy fallback validation.

use crate::copy::CopyConflictPolicy;
use crate::copy::CopyOptions;
use crate::copy::FallbackRejection;
use crate::copy::MetadataPreservePolicy;
use crate::copy::ServerSidePreference;
use crate::error::FsError;
use crate::metadata::AtomicityRequirement;
use crate::metadata::DurabilityRequirement;
use crate::metadata::FileSystemLimits;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;
use crate::write::WriteDisposition;
use crate::write::WriteOptions;

/// Returns true when copy options remain within the fallback policy allowlist.
///
/// # Parameters
///
/// - `options`: Copy route, conflict, consistency, and metadata guarantees
///   requested by the caller.
/// - `filesystem_symlink_policy`: Symlink policy configured for the filesystem.
#[inline]
pub(crate) fn fallback_options_supported(options: &CopyOptions, filesystem_symlink_policy: SymlinkPolicy) -> bool {
    fallback_rejection(options, filesystem_symlink_policy).is_none()
}

/// Returns the first static reason the stream fallback is unavailable.
///
/// # Parameters
///
/// - `options`: Copy route, conflict, consistency, and metadata guarantees
///   requested by the caller.
/// - `filesystem_symlink_policy`: Symlink policy configured for the filesystem.
pub(crate) fn fallback_rejection(
    options: &CopyOptions,
    filesystem_symlink_policy: SymlinkPolicy,
) -> Option<FallbackRejection> {
    if matches!(options.mode(), crate::copy::CopyMode::Tree) {
        return Some(FallbackRejection::TreeMode);
    }
    if options
        .symlink_policy_override()
        .is_some_and(|policy| policy != filesystem_symlink_policy)
    {
        return Some(FallbackRejection::SymlinkPolicyOverride);
    }
    if options.continue_on_error() {
        return Some(FallbackRejection::ContinueOnError);
    }
    if options.preserve_metadata() != MetadataPreservePolicy::None {
        return Some(FallbackRejection::MetadataPreservation);
    }
    if options.server_side() == ServerSidePreference::Require {
        return Some(FallbackRejection::ServerSideRequired);
    }
    if options.create_parent() {
        return Some(FallbackRejection::CreateParent);
    }
    if options.durability() == DurabilityRequirement::Required {
        return Some(FallbackRejection::DurabilityRequired);
    }
    if options.conflict() == CopyConflictPolicy::Skip && options.atomicity() == AtomicityRequirement::Required {
        return Some(FallbackRejection::AtomicSkip);
    }
    if !matches!(options.conflict(), CopyConflictPolicy::Fail | CopyConflictPolicy::Skip) {
        return Some(FallbackRejection::ConflictPolicy);
    }
    None
}

/// Builds the writer request used by a streamed copy fallback.
///
/// # Parameters
///
/// - `options`: Copy route, conflict, consistency, and metadata guarantees
///   requested by the caller.
#[inline]
#[must_use]
pub(crate) fn fallback_write_options(options: &CopyOptions) -> WriteOptions {
    WriteOptions::default()
        .with_disposition(WriteDisposition::CreateNew)
        .with_atomicity(options.atomicity())
        .with_durability(options.durability())
}

/// Validates stream-copy read/write size constraints using the provided limits.
///
/// # Parameters
///
/// - `limits`: Provider resource limits applied to the operation.
/// - `_source`: Source-side limits used to check the requested copy length.
/// - `target`: Destination logical path affected by the operation.
/// - `length`: Length of the source resource to validate against copy limits.
pub(crate) fn validate_stream_copy_length_limits(
    limits: &FileSystemLimits,
    _source: &Path,
    target: &Path,
    length: u64,
) -> Result<(), FsError> {
    limits.validate_write_size_u64(target, length)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::fallback_options_supported;
    use crate::copy::CopyOptions;
    use crate::metadata::SymlinkPolicy;

    #[test]
    fn test_fallback_rejects_tree_mode() {
        assert!(!fallback_options_supported(&CopyOptions::tree(), SymlinkPolicy::Reject,));
    }

    #[test]
    fn test_fallback_rejects_symlink_policy_override_that_provider_cannot_honor() {
        assert!(!fallback_options_supported(
            &CopyOptions::default().with_symlink_policy(SymlinkPolicy::FollowWithinFileSystem),
            SymlinkPolicy::Reject,
        ));
        assert!(fallback_options_supported(
            &CopyOptions::default().with_symlink_policy(SymlinkPolicy::Reject),
            SymlinkPolicy::Reject,
        ));
    }
}
