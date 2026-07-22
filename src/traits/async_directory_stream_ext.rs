// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Convenience methods for asynchronous directory streams.

use crate::{
    AsyncDirectoryStream,
    DirEntry,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
};

/// Future-based convenience methods for asynchronous directory streams.
pub trait AsyncDirectoryStreamExt {
    /// Collects all remaining entries asynchronously.
    ///
    /// # Parameters
    ///
    /// - `max_entries`: Maximum number of entries to retain in memory.
    ///
    /// # Returns
    ///
    /// A future resolving to at most `max_entries` remaining entries.
    ///
    /// # Errors
    ///
    /// The future resolves to an error when listing fails before the stream
    /// ends or when more than `max_entries` entries remain.
    fn collect_entries_async(
        self,
        max_entries: usize,
    ) -> FsFuture<'static, Vec<DirEntry>>;
}

impl AsyncDirectoryStreamExt for AsyncDirectoryStream {
    fn collect_entries_async(
        mut self,
        max_entries: usize,
    ) -> FsFuture<'static, Vec<DirEntry>> {
        Box::pin(async move {
            let mut entries = Vec::new();
            while entries.len() < max_entries {
                match self.next_entry_async().await? {
                    Some(entry) => entries.push(entry),
                    None => return Ok(entries),
                }
            }
            match self.next_entry_async().await? {
                Some(_) => Err(FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::List,
                    "directory listing exceeds the caller entry limit",
                )),
                None => Ok(entries),
            }
        })
    }
}
