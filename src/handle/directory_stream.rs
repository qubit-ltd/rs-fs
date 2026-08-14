// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Concrete synchronous directory stream handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::DirEntry;
use crate::DirectoryStreamState;
use crate::FileSystemLimits;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::FsResult;
use crate::ListOptions;
use crate::Path;
use crate::handle::directory_entry_validation;
use crate::spi::DirectoryStreamSpi;

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
    state: DirectoryStreamState,
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
            state: DirectoryStreamState::Open,
        }
    }

    /// Returns the current lifecycle state of this stream.
    #[inline]
    #[must_use = "inspect the stream lifecycle state"]
    pub const fn state(&self) -> DirectoryStreamState {
        self.state
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
        if self.state != DirectoryStreamState::Open {
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
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.contextual_error(error));
                }
                if let Err(message) =
                    directory_entry_validation::matches_options(
                        &entry,
                        &self.root,
                        &self.options,
                    )
                {
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.contextual_error(
                        directory_entry_validation::option_error(
                            &self.root, message,
                        ),
                    ));
                }
                Ok(Some(entry))
            }
            Ok(None) => {
                self.state = DirectoryStreamState::Exhausted;
                Ok(None)
            }
            Err(error) => {
                self.state = DirectoryStreamState::Failed;
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
