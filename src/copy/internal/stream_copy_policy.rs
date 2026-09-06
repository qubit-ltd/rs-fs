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
use crate::metadata::FileKind;
use crate::metadata::FileSystemLimits;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;

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

/// Validates stream-copy read/write size constraints using the provided limits.
pub(crate) fn validate_stream_copy_length_limits(
    limits: &FileSystemLimits,
    source: &Path,
    target: &Path,
    length: u64,
) -> Result<(), FsError> {
    let length = usize::try_from(length).map_err(|_| {
        FsError::new(
            crate::error::FsErrorKind::ResourceLimitExceeded,
            crate::error::FsOperation::Copy,
            "source length cannot fit in a native write session",
        )
        .with_path(source.clone())
    })?;
    limits.validate_write_size(target, length)?;
    Ok(())
}

#[inline]
pub(crate) fn is_file_kind_supported(kind: FileKind) -> bool {
    matches!(kind, FileKind::File | FileKind::Object)
}

#[cfg(test)]
mod tests {
    use super::*;

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
