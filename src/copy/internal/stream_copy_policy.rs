// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared helpers for stream-copy fallback validation.

use crate::AtomicityRequirement;
use crate::CopyConflictPolicy;
use crate::CopyOptions;
use crate::DurabilityRequirement;
use crate::FileKind;
use crate::FileSystemLimits;
use crate::FsError;
use crate::MetadataPreservePolicy;
use crate::Path;
use crate::ServerSidePreference;

/// Returns true when copy options remain within the fallback policy allowlist.
#[inline]
pub(crate) fn fallback_options_supported(options: &CopyOptions) -> bool {
    !options.continue_on_error()
        && options.preserve_metadata() == MetadataPreservePolicy::None
        && options.server_side() != ServerSidePreference::Require
        && !options.create_parent()
        && options.durability() != DurabilityRequirement::Required
        && !(options.conflict() == CopyConflictPolicy::Skip
            && options.atomicity() == AtomicityRequirement::Required)
        && matches!(
            options.conflict(),
            CopyConflictPolicy::Fail | CopyConflictPolicy::Skip
        )
}

/// Validates stream-copy read/write size constraints using the provided limits.
pub(crate) fn validate_stream_copy_length_limits(
    limits: &FileSystemLimits,
    source: &Path,
    target: &Path,
    length: u64,
) -> Result<(), FsError> {
    limits.validate_read_range(source, Some(length))?;
    let length = usize::try_from(length).map_err(|_| {
        FsError::new(
            crate::FsErrorKind::ResourceLimitExceeded,
            crate::FsOperation::Copy,
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
