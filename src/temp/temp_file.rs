// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous temporary file handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FileReader,
    FileResource,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    PersistFailure,
    PersistFailureState,
    PersistOptions,
    PersistOutcome,
    ReadOptions,
    TempResourceSession,
    TempResourceState,
    WriteOptions,
};

/// Type-erased temporary file retaining cleanup responsibility after failures.
pub struct TempFile {
    resource: FileResource,
    session: Box<dyn TempResourceSession>,
    state: TempResourceState,
}

impl TempFile {
    /// Creates a temporary file from its ordinary resource and lifecycle
    /// session.
    ///
    /// # Parameters
    /// - `resource`: Addressable temporary file resource.
    /// - `session`: Provider lifecycle session owning cleanup responsibility.
    ///
    /// # Returns
    /// A concrete temporary file in [`TempResourceState::Owned`].
    #[inline]
    #[must_use]
    pub fn new<S>(resource: FileResource, session: S) -> Self
    where
        S: TempResourceSession + 'static,
    {
        Self {
            resource,
            session: Box::new(session),
            state: TempResourceState::Owned,
        }
    }

    /// Returns the temporary provider-local path.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        self.resource.path()
    }

    /// Returns the ordinary file resource associated with this handle.
    #[inline]
    #[must_use]
    pub fn resource(&self) -> &FileResource {
        &self.resource
    }

    /// Returns the current ownership and recovery state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.state
    }

    /// Opens the temporary file for synchronous reading.
    ///
    /// # Errors
    /// Returns the owning filesystem's open error.
    #[inline]
    pub fn open_reader(&self, options: ReadOptions) -> FsResult<FileReader> {
        self.resource.open_reader(options)
    }

    /// Opens the temporary file for synchronous writing.
    ///
    /// # Errors
    /// Returns the owning filesystem's open error.
    #[inline]
    pub fn open_writer(&self, options: WriteOptions) -> FsResult<FileWriter> {
        self.resource.open_writer(options)
    }

    /// Explicitly deletes the source and releases cleanup responsibility.
    ///
    /// # Errors
    /// Returns invalid-state after persistence, keep, or cleanup. A provider
    /// cleanup failure leaves the handle available with
    /// [`TempResourceState::CleanupRequired`], unless the provider reports an
    /// indeterminate result.
    pub fn cleanup(&mut self) -> FsResult<()> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            return Err(self.invalid_state(
                FsOperation::CleanupTemp,
                "temporary file cannot be cleaned now",
            ));
        }
        match self.session.cleanup() {
            Ok(()) => {
                self.state = TempResourceState::Cleaned;
                Ok(())
            }
            Err(error) => {
                self.state = if error.kind() == FsErrorKind::Indeterminate {
                    TempResourceState::Indeterminate
                } else {
                    TempResourceState::CleanupRequired
                };
                Err(error)
            }
        }
    }

    /// Keeps the source and transfers cleanup responsibility to the caller.
    ///
    /// # Returns
    /// The retained provider-local source path.
    ///
    /// # Errors
    /// Returns invalid-state or the provider ownership-transfer error. An
    /// indeterminate error disables automatic cleanup because ownership may
    /// already have transferred.
    pub fn keep(&mut self) -> FsResult<FsPath> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            return Err(self.invalid_state(
                FsOperation::KeepTemp,
                "temporary file cannot be kept now",
            ));
        }
        match self.session.keep() {
            Ok(()) => {
                self.state = TempResourceState::Kept;
                Ok(self.path().clone())
            }
            Err(error) => {
                if error.kind() == FsErrorKind::Indeterminate {
                    self.state = TempResourceState::Indeterminate;
                }
                Err(error)
            }
        }
    }

    /// Publishes this temporary file without consuming the handle.
    ///
    /// Required atomicity is checked against the stable filesystem capability
    /// snapshot before the provider session is called. On success the handle
    /// becomes persisted. On failure it remains available with recovery state:
    ///
    /// - [`PersistFailureState::NotPublished`] keeps the state owned and allows
    ///   a corrected retry or explicit cleanup;
    /// - [`PersistFailureState::PublishedSourceRetained`] records partial
    ///   success and keeps source cleanup responsibility in this handle;
    /// - [`PersistFailureState::Indeterminate`] disables automatic cleanup and
    ///   publication retry because either source or target may have changed.
    ///
    /// Dropping an owned or cleanup-required synchronous handle performs a
    /// best-effort source cleanup. Drop never cleans an indeterminate handle.
    /// Callers needing confirmed cleanup must call [`Self::cleanup`]
    /// explicitly.
    ///
    /// # Parameters
    /// - `target`: Final provider-local target path.
    /// - `options`: Persistence requirements.
    ///
    /// # Returns
    /// The confirmed actual publication method and atomicity.
    pub fn persist(
        &mut self,
        target: &FsPath,
        options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure> {
        if self.state != TempResourceState::Owned {
            return Err(PersistFailure::new(
                self.invalid_state(
                    FsOperation::PersistTemp,
                    "temporary file cannot be persisted now",
                ),
                PersistFailureState::NotPublished,
            ));
        }
        if let Err(error) = self
            .resource
            .validate_path(target, FsOperation::PersistTemp)
        {
            return Err(PersistFailure::new(
                error
                    .with_path(self.path().clone())
                    .with_target(target.clone()),
                PersistFailureState::NotPublished,
            ));
        }
        if let Err(error) =
            options.validate_against(self.resource.fs().capabilities())
        {
            return Err(PersistFailure::new(
                error
                    .with_path(self.path().clone())
                    .with_target(target.clone()),
                PersistFailureState::NotPublished,
            ));
        }
        match self.session.persist(target, options) {
            Ok(outcome) => {
                self.state = TempResourceState::Persisted;
                Ok(outcome)
            }
            Err(failure) => {
                self.state = match failure.state() {
                    PersistFailureState::NotPublished => {
                        TempResourceState::Owned
                    }
                    PersistFailureState::PublishedSourceRetained => {
                        TempResourceState::CleanupRequired
                    }
                    PersistFailureState::Indeterminate => {
                        TempResourceState::Indeterminate
                    }
                };
                Err(failure)
            }
        }
    }

    /// Builds an invalid-state error for this temporary file.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.path().clone())
    }
}

impl Debug for TempFile {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("TempFile")
            .field("resource", &self.resource)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let _ = self.session.cleanup();
        }
    }
}
