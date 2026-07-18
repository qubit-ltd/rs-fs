// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete asynchronous directory stream handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::{
    AsyncDirectoryStreamSession,
    DirEntry,
    FsFuture,
};

/// Type-erased asynchronous directory enumeration handle.
pub struct AsyncDirectoryStream {
    session: Box<dyn AsyncDirectoryStreamSession>,
}

impl AsyncDirectoryStream {
    /// Wraps an already-open asynchronous provider enumeration session.
    ///
    /// # Parameters
    /// - `session`: Provider asynchronous enumeration session.
    ///
    /// # Returns
    /// A concrete type-erased asynchronous directory stream.
    #[inline]
    #[must_use]
    pub fn new<S>(session: S) -> Self
    where
        S: AsyncDirectoryStreamSession + 'static,
    {
        Self {
            session: Box::new(session),
        }
    }

    /// Asynchronously reads the next directory entry.
    ///
    /// # Returns
    /// A future resolving to one entry or `None` at end of enumeration.
    #[inline]
    pub fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        self.session.next_entry_async()
    }
}

impl Debug for AsyncDirectoryStream {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncDirectoryStream")
            .finish_non_exhaustive()
    }
}
