// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete asynchronous directory stream handle.

use std::fmt::{Debug, Formatter, Result as FmtResult};

use crate::spi::{AsyncDirectoryStreamSession, SpiFuture};
use crate::{DirEntry, FsError, FsErrorKind, FsOperation, FsResult, Path};

/// Type-erased asynchronous directory enumeration handle.
pub struct AsyncDirectoryStream {
    session: Box<dyn AsyncDirectoryStreamSession>,
    root: Path,
    terminal: bool,
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
    pub(crate) fn new(root: Path, session: Box<dyn AsyncDirectoryStreamSession>) -> Self {
        Self {
            session,
            root,
            terminal: false,
        }
    }

    /// Asynchronously reads the next directory entry.
    ///
    /// # Returns
    /// A future resolving to one entry or `None` at end of enumeration.
    pub fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        if self.terminal {
            return Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::InvalidState,
                    FsOperation::List,
                    "directory stream is terminal",
                ))
            });
        }
        Box::pin(async move {
            match self.session.next_entry_async().await {
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
        })
    }
}

/// Returns whether an entry remains inside the stream's requested logical root.
fn is_within(root: &Path, entry: &Path) -> bool {
    root == entry
        || (entry.as_str().starts_with(root.as_str())
            && (root.as_str() == "/"
                || entry.as_str().as_bytes().get(root.as_str().len()) == Some(&b'/')))
}

impl Debug for AsyncDirectoryStream {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncDirectoryStream")
            .finish_non_exhaustive()
    }
}
