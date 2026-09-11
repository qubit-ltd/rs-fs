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
#[inline]
pub(crate) fn fallback_options_supported(options: &CopyOptions, filesystem_symlink_policy: SymlinkPolicy) -> bool {
    !matches!(options.mode(), crate::copy::CopyMode::Tree)
        && options
            .symlink_policy_override()
            .is_none_or(|policy| policy == filesystem_symlink_policy)
        && !options.continue_on_error()
        && options.preserve_metadata() == MetadataPreservePolicy::None
        && options.server_side() != ServerSidePreference::Require
        && !options.create_parent()
        && options.durability() != DurabilityRequirement::Required
        && !(options.conflict() == CopyConflictPolicy::Skip && options.atomicity() == AtomicityRequirement::Required)
        && matches!(options.conflict(), CopyConflictPolicy::Fail | CopyConflictPolicy::Skip)
}

/// Builds the writer request used by a streamed copy fallback.
#[inline]
pub(crate) fn fallback_write_options(options: &CopyOptions) -> WriteOptions {
    WriteOptions::default()
        .with_disposition(WriteDisposition::CreateNew)
        .with_atomicity(options.atomicity())
        .with_durability(options.durability())
}

/// Validates stream-copy read/write size constraints using the provided limits.
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
    fn fallback_rejects_tree_mode() {
        assert!(!fallback_options_supported(&CopyOptions::tree(), SymlinkPolicy::Reject,));
    }

    #[test]
    fn fallback_rejects_symlink_policy_override_that_provider_cannot_honor() {
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
