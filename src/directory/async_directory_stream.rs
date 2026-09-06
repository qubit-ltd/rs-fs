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
use std::time::Instant;

use crate::directory::DirectoryStreamState;
use crate::directory::ListOptions;
use crate::directory::directory_entry_validation;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::DirEntry;
use crate::metadata::FileSystemLimits;
use crate::path::Path;
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
    path_semantics: crate::path::PathSemantics,
    /// Provider path limits used to validate every returned entry.
    limits: FileSystemLimits,
    /// Whether enumeration has completed or encountered a terminal failure.
    state: DirectoryStreamState,
    /// Monotonic deadline computed when the stream is created.
    deadline: Option<Instant>,
    /// Number of entries already returned to the caller.
    returned_entries: usize,
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
        path_semantics: crate::path::PathSemantics,
        limits: FileSystemLimits,
    ) -> Self {
        let deadline = options
            .deadline()
            .and_then(|duration| Instant::now().checked_add(duration));
        Self {
            session,
            root,
            options,
            provider: provider.into(),
            path_semantics,
            limits,
            state: DirectoryStreamState::Open,
            deadline,
            returned_entries: 0,
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
    pub fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        if self.state != DirectoryStreamState::Open {
            return Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::InvalidState,
                    FsOperation::List,
                    "directory stream is terminal",
                ))
            });
        }
        if self.deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            self.state = DirectoryStreamState::Failed;
            let error = self.resource_limit_error("directory listing deadline was exceeded");
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            match self.session.next_entry_async().await {
                Ok(Some(entry)) => {
                    if let Err(error) =
                        directory_entry_validation::validate_entry(&entry, &self.root, self.path_semantics, self.limits)
                    {
                        self.state = DirectoryStreamState::Failed;
                        return Err(self.contextual_error(error));
                    }
                    if let Err(message) =
                        crate::directory::internal::select(&entry, &self.root, &self.options, self.path_semantics)
                    {
                        self.state = DirectoryStreamState::Failed;
                        return Err(
                            self.contextual_error(directory_entry_validation::option_error(&self.root, message))
                        );
                    }
                    if self.options.max_depth().is_some_and(|maximum| {
                        directory_entry_validation::entry_depth(&self.root, &entry.path)
                            .is_some_and(|depth| depth > maximum)
                    }) {
                        self.state = DirectoryStreamState::Failed;
                        return Err(self.resource_limit_error("directory listing depth limit was exceeded"));
                    }
                    if self
                        .options
                        .max_entries()
                        .is_some_and(|maximum| self.returned_entries >= maximum)
                    {
                        self.state = DirectoryStreamState::Failed;
                        return Err(self.resource_limit_error("directory listing entry limit was exceeded"));
                    }
                    self.returned_entries = self.returned_entries.checked_add(1).ok_or_else(|| {
                        self.state = DirectoryStreamState::Failed;
                        self.resource_limit_error("directory listing entry count exceeded the API range")
                    })?;
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

    /// Builds a terminal caller-budget error with list context.
    fn resource_limit_error(&self, message: &str) -> FsError {
        self.contextual_error(FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::List,
            message,
        ))
    }
}

impl Debug for AsyncDirectoryStream {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.debug_struct("AsyncDirectoryStream").finish_non_exhaustive()
    }
}
