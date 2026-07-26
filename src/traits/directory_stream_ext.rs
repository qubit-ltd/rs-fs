// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience methods for directory streams.

use crate::{DirEntry, DirectoryStream, FsError, FsErrorKind, FsOperation, FsResult};

/// Convenience methods for directory streams.
pub trait DirectoryStreamExt {
    /// Collects all remaining entries.
    ///
    /// # Parameters
    /// - `max_entries`: Maximum number of entries to retain in memory.
    ///
    /// # Returns
    /// Entries produced by the stream.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when listing fails before the stream ends or
    /// when more than `max_entries` entries remain.
    fn collect_entries(self, max_entries: usize) -> FsResult<Vec<DirEntry>>;
}

impl DirectoryStreamExt for DirectoryStream {
    fn collect_entries(mut self, max_entries: usize) -> FsResult<Vec<DirEntry>> {
        let mut entries = Vec::new();
        while entries.len() < max_entries {
            match self.next_entry()? {
                Some(entry) => entries.push(entry),
                None => return Ok(entries),
            }
        }
        match self.next_entry()? {
            Some(_) => Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::List,
                "directory listing exceeds the caller entry limit",
            )),
            None => Ok(entries),
        }
    }
}
