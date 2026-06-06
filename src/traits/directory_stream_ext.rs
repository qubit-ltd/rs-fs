// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience methods for directory streams.

use crate::{
    DirEntry,
    DirectoryStream,
    FsResult,
};

/// Convenience methods for directory streams.
pub trait DirectoryStreamExt {
    /// Collects all remaining entries.
    ///
    /// # Returns
    /// Entries produced by the stream.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when listing fails before the stream ends.
    fn collect_entries(self) -> FsResult<Vec<DirEntry>>;
}

impl DirectoryStreamExt for Box<dyn DirectoryStream> {
    fn collect_entries(mut self) -> FsResult<Vec<DirEntry>> {
        let mut entries = Vec::new();
        while let Some(entry) = self.next_entry()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}
