// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Concrete synchronous directory stream handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::handle::directory_entry_validation;
use crate::spi::DirectoryStreamSpi;
use crate::{
    DirEntry,
    FileSystemLimits,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    ListOptions,
    Path,
};

/// Type-erased synchronous directory enumeration handle.
pub struct DirectoryStream {
    /// Provider enumeration session.
    session: Box<dyn DirectoryStreamSpi>,
    /// Validated root constraining returned entries.
    root: Path,
    /// Listing policy used to validate provider results.
    options: ListOptions,
    /// Provider identifier attached to facade-generated errors.
    provider: Box<str>,
    /// Provider path semantics used to validate every returned entry.
    path_semantics: crate::PathSemantics,
    /// Provider path limits used to validate every returned entry.
    limits: FileSystemLimits,
    /// Whether enumeration has completed or encountered a terminal failure.
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
        path_semantics: crate::PathSemantics,
        limits: FileSystemLimits,
    ) -> Self {
        Self {
            session,
            root,
            options,
            provider: provider.into(),
            path_semantics,
            limits,
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
            Ok(Some(entry)) => {
                if let Err(error) = directory_entry_validation::validate_entry(
                    &entry,
                    &self.root,
                    self.path_semantics,
                    self.limits,
                ) {
                    self.terminal = true;
                    return Err(self.contextual_error(error));
                }
                if directory_entry_validation::matches_options(
                    &entry,
                    &self.root,
                    &self.options,
                ) {
                    return Ok(Some(entry));
                }
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

    /// Adds only missing facade facts to a provider stream error.
    fn contextual_error(&self, error: FsError) -> FsError {
        error
            .with_operation(FsOperation::List)
            .with_missing_context(&self.root, None, &self.provider)
    }
}

impl Debug for DirectoryStream {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("DirectoryStream")
            .finish_non_exhaustive()
    }
}
