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
use crate::{DirEntry, FsError, FsErrorKind, FsOperation, FsResult, ListOptions, Path};

/// Type-erased synchronous directory enumeration handle.
pub struct DirectoryStream {
    session: Box<dyn DirectoryStreamSpi>,
    root: Path,
    options: ListOptions,
    provider: Box<str>,
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
    pub(crate) fn new(
        root: Path,
        session: Box<dyn DirectoryStreamSpi>,
        options: ListOptions,
        provider: &str,
    ) -> Self {
        Self {
            session,
            root,
            options,
            provider: provider.into(),
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
            Ok(Some(entry)) if self.entry_satisfies_options(&entry) => Ok(Some(entry)),
            Ok(Some(_)) => {
                self.terminal = true;
                Err(self.contextual_error(
                    FsError::new(
                        FsErrorKind::ProviderContractViolation,
                        FsOperation::ValidateProviderOutcome,
                        "provider returned directory entry outside requested root",
                    )
                    .with_path(self.root.clone()),
                ))
            }
            Ok(None) => {
                self.terminal = true;
                Ok(None)
            }
            Err(error) => {
                self.terminal = true;
                Err(self.contextual_error(error))
            }
        }
    }

    /// Checks one provider entry against the request retained by this stream.
    fn entry_satisfies_options(&self, entry: &DirEntry) -> bool {
        let Some(relative) = relative_path(&self.root, &entry.path) else {
            return false;
        };
        if !self.options.recursive && self.options.prefix.is_none() && relative.contains('/') {
            return false;
        }
        if self.options.include_metadata && entry.metadata.is_none() {
            return false;
        }
        self.options.prefix.as_deref().is_none_or(|prefix| {
            relative == prefix
                || relative
                    .strip_prefix(prefix)
                    .is_some_and(|remaining| remaining.starts_with('/'))
        })
    }

    /// Adds only missing facade facts to a provider stream error.
    fn contextual_error(&self, error: FsError) -> FsError {
        error
            .with_operation(FsOperation::List)
            .with_missing_context(&self.root, None, &self.provider)
    }
}

/// Returns the entry path relative to `root` when it remains in the root.
fn relative_path<'a>(root: &Path, entry: &'a Path) -> Option<&'a str> {
    if root == entry {
        Some("")
    } else if root.as_str() == "/" {
        entry.as_str().strip_prefix('/')
    } else {
        entry
            .as_str()
            .strip_prefix(root.as_str())?
            .strip_prefix('/')
    }
}

impl Debug for DirectoryStream {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("DirectoryStream")
            .finish_non_exhaustive()
    }
}
