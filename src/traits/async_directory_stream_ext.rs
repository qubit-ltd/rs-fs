// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Convenience methods for asynchronous directory streams.

use crate::{
    AsyncDirectoryStream,
    DirEntry,
    FsFuture,
};

/// Future-based convenience methods for asynchronous directory streams.
pub trait AsyncDirectoryStreamExt {
    /// Collects all remaining entries asynchronously.
    ///
    /// This helper is appropriate only when the caller intentionally accepts
    /// memory use proportional to the complete remaining enumeration.
    ///
    /// # Returns
    ///
    /// A future resolving to all remaining entries.
    fn collect_entries_async(self) -> FsFuture<'static, Vec<DirEntry>>;
}

impl AsyncDirectoryStreamExt for AsyncDirectoryStream {
    fn collect_entries_async(mut self) -> FsFuture<'static, Vec<DirEntry>> {
        Box::pin(async move {
            let mut entries = Vec::new();
            while let Some(entry) = self.next_entry_async().await? {
                entries.push(entry);
            }
            Ok(entries)
        })
    }
}
