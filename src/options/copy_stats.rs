// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation statistics.

use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::FsResult;

/// Statistics collected during copy operations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CopyStats {
    /// Number of regular files copied.
    pub files: u64,
    /// Number of directories created or copied.
    pub directories: u64,
    /// Number of symbolic links copied.
    pub symlinks: u64,
    /// Number of object-store objects copied.
    pub objects: u64,
    /// Number of object-store prefixes or collection resources copied.
    pub prefixes: u64,
    /// Number of content bytes copied.
    pub bytes: u64,
    /// Number of destination entries overwritten.
    pub overwritten: u64,
    /// Number of entries skipped.
    pub skipped: u64,
    /// Number of failed entries when continue-on-error is enabled.
    pub failed: u64,
}

impl CopyStats {
    /// Adds another statistics value into this one.
    ///
    /// # Parameters
    /// - `other`: Statistics to add.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::ResourceLimitExceeded`] when any counter would
    /// overflow `u64`. The receiver remains unchanged on overflow.
    #[inline]
    pub fn add_assign(&mut self, other: &Self) -> FsResult<()> {
        let add = |left: u64, right: u64| {
            left.checked_add(right).ok_or_else(|| {
                FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Copy,
                    "copy statistics counter overflow",
                )
            })
        };
        let files = add(self.files, other.files)?;
        let directories = add(self.directories, other.directories)?;
        let symlinks = add(self.symlinks, other.symlinks)?;
        let objects = add(self.objects, other.objects)?;
        let prefixes = add(self.prefixes, other.prefixes)?;
        let bytes = add(self.bytes, other.bytes)?;
        let overwritten = add(self.overwritten, other.overwritten)?;
        let skipped = add(self.skipped, other.skipped)?;
        let failed = add(self.failed, other.failed)?;
        *self = Self {
            files,
            directories,
            symlinks,
            objects,
            prefixes,
            bytes,
            overwritten,
            skipped,
            failed,
        };
        Ok(())
    }
}
