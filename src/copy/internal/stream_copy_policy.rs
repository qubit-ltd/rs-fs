// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared helpers for stream-copy fallback validation.

use crate::{
    AtomicityRequirement,
    CopyConflictPolicy,
    CopyOptions,
    DurabilityRequirement,
    FileKind,
    FileSystemLimits,
    FsError,
    MetadataPreservePolicy,
    Path,
    ServerSidePreference,
};

/// Returns true when copy options remain within the fallback policy allowlist.
#[inline]
pub(crate) fn fallback_options_supported(options: &CopyOptions) -> bool {
    !options.continue_on_error
        && options.preserve_metadata == MetadataPreservePolicy::None
        && options.server_side != ServerSidePreference::Require
        && !options.create_parent
        && options.durability != DurabilityRequirement::Required
        && !(options.conflict == CopyConflictPolicy::Skip
            && options.atomicity == AtomicityRequirement::Required)
        && matches!(
            options.conflict,
            CopyConflictPolicy::Fail | CopyConflictPolicy::Skip
        )
}

#[inline]
#[cfg(not(target_pointer_width = "64"))]
fn length_as_usize(length: u64) -> Option<usize> {
    usize::try_from(length).ok()
}

/// Validates stream-copy read/write size constraints using the provided limits.
pub(crate) fn validate_stream_copy_length_limits(
    limits: &FileSystemLimits,
    source: &Path,
    target: &Path,
    length: u64,
) -> Result<(), FsError> {
    limits.validate_read_range(source, Some(length))?;
    #[cfg(target_pointer_width = "64")]
    let length = length as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let length = match length_as_usize(length) {
        Some(length) => length,
        None => {
            return Err(FsError::new(
                crate::FsErrorKind::ResourceLimitExceeded,
                crate::FsOperation::Copy,
                "source length cannot fit in a write session",
            )
            .with_path(source.clone()));
        }
    };
    limits.validate_write_size(target, length)?;
    Ok(())
}

#[inline]
pub(crate) fn is_file_kind_supported(kind: FileKind) -> bool {
    matches!(kind, FileKind::File | FileKind::Object)
}
