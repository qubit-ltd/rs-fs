// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Concrete synchronous directory stream handle.

use std::fmt::{Debug, Formatter, Result as FmtResult};

use crate::{DirEntry, DirectoryStreamSession, FsResult};

/// Type-erased synchronous directory enumeration handle.
pub struct DirectoryStream {
    session: Box<dyn DirectoryStreamSession>,
}

impl DirectoryStream {
    /// Wraps an already-open provider enumeration session.
    ///
    /// # Parameters
    /// - `session`: Provider directory enumeration session.
    ///
    /// # Returns
    /// A concrete type-erased directory stream.
    #[inline]
    #[must_use]
    pub fn new<S>(session: S) -> Self
    where
        S: DirectoryStreamSession + 'static,
    {
        Self {
            session: Box::new(session),
        }
    }

    /// Reads the next directory entry.
    ///
    /// # Returns
    /// `Some` for one entry or `None` at end of enumeration.
    ///
    /// # Errors
    /// Returns a filesystem error when enumeration cannot continue.
    #[inline]
    pub fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        self.session.next_entry()
    }
}

impl Debug for DirectoryStream {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("DirectoryStream")
            .finish_non_exhaustive()
    }
}
