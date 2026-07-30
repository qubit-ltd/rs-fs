// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy operation statistics.

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
    #[inline]
    pub fn add_assign(&mut self, other: &Self) {
        self.files += other.files;
        self.directories += other.directories;
        self.symlinks += other.symlinks;
        self.objects += other.objects;
        self.prefixes += other.prefixes;
        self.bytes += other.bytes;
        self.overwritten += other.overwritten;
        self.skipped += other.skipped;
        self.failed += other.failed;
    }
}
