// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Concrete asynchronous directory stream handle.

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
use crate::spi::AsyncDirectoryStreamSession;
use crate::spi::SpiFuture;

/// Type-erased asynchronous directory enumeration handle.
pub struct AsyncDirectoryStream {
    /// Provider enumeration session.
    session: Box<dyn AsyncDirectoryStreamSession>,
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
    pub(crate) fn new(
        root: Path,
        session: Box<dyn AsyncDirectoryStreamSession>,
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

    /// Asynchronously reads the next directory entry.
    ///
    /// # Returns
    /// A future resolving to one entry or `None` at end of enumeration.
    pub fn next_entry_async(
        &mut self,
    ) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        if self.state != DirectoryStreamState::Open {
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
                Ok(Some(entry)) => {
                    if let Err(error) =
                        directory_entry_validation::validate_entry(
                            &entry,
                            &self.root,
                            self.path_semantics,
                            self.limits,
                        )
                    {
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
        })
    }

    /// Adds only missing facade facts to a provider stream error.
    fn contextual_error(&self, error: FsError) -> FsError {
        error
            .with_operation(FsOperation::List)
            .with_missing_context(&self.root, None, &self.provider)
    }
}

impl Debug for AsyncDirectoryStream {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncDirectoryStream")
            .finish_non_exhaustive()
    }
}
