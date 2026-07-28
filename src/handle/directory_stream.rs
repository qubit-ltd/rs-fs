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

use crate::spi::DirectoryStreamSpi;
use crate::{DirEntry, FsError, FsErrorKind, FsOperation, FsResult, Path};

/// Type-erased synchronous directory enumeration handle.
pub struct DirectoryStream {
    session: Box<dyn DirectoryStreamSpi>,
    root: Path,
    terminal: bool,
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
    pub(crate) fn new(root: Path, session: Box<dyn DirectoryStreamSpi>) -> Self {
        Self {
            session,
            root,
            terminal: false,
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
        if self.terminal {
            return Err(FsError::new(
                FsErrorKind::InvalidState,
                FsOperation::List,
                "directory stream is terminal",
            ));
        }
        match self.session.next_entry() {
            Ok(Some(entry)) if is_within(&self.root, &entry.path) => Ok(Some(entry)),
            Ok(Some(_)) => {
                self.terminal = true;
                Err(FsError::new(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::ValidateProviderOutcome,
                    "provider returned directory entry outside requested root",
                )
                .with_path(self.root.clone()))
            }
            Ok(None) => {
                self.terminal = true;
                Ok(None)
            }
            Err(error) => {
                self.terminal = true;
                Err(error)
            }
        }
    }
}

/// Returns whether a provider entry is within the requested logical namespace.
fn is_within(root: &Path, entry: &Path) -> bool {
    root == entry
        || (entry.as_str().starts_with(root.as_str())
            && (root.as_str() == "/"
                || entry.as_str().as_bytes().get(root.as_str().len()) == Some(&b'/')))
}

impl Debug for DirectoryStream {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("DirectoryStream")
            .finish_non_exhaustive()
    }
}
