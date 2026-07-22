// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous temporary directory handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use log::warn;

use crate::{
    CreateDirOptions,
    DirectoryStream,
    FileResource,
    FsError,
    FsErrorKind,
    FsName,
    FsOperation,
    FsPath,
    FsResult,
    ListOptions,
    PersistFailure,
    PersistFailureState,
    PersistOptions,
    PersistOutcome,
    RelativeFsPath,
    TempResourceSession,
    TempResourceState,
};

/// Type-erased temporary directory retaining cleanup responsibility.
pub struct TempDir {
    resource: FileResource,
    session: Box<dyn TempResourceSession>,
    state: TempResourceState,
}

impl TempDir {
    /// Creates a temporary directory from its resource and lifecycle session.
    ///
    /// # Parameters
    /// - `resource`: Addressable temporary directory resource.
    /// - `session`: Provider lifecycle session owning cleanup responsibility.
    ///
    /// # Returns
    /// A concrete temporary directory in [`TempResourceState::Owned`].
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

    /// Returns the provider-local temporary directory path.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        self.resource.path()
    }

    /// Returns the ordinary resource associated with this handle.
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

    /// Lists entries under the temporary directory.
    ///
    /// # Errors
    /// Returns the owning filesystem's listing error.
    #[inline]
    pub fn list(&self, options: ListOptions) -> FsResult<DirectoryStream> {
        self.resource.fs().list(self.path(), options)
    }

    /// Builds one immediate child resource using a validated single name.
    ///
    /// Absolute paths, separators, `.` and `..` cannot be represented by
    /// [`FsName`], so this operation cannot escape the temporary directory by
    /// lexical path replacement.
    ///
    /// # Parameters
    /// - `name`: Validated single child name.
    ///
    /// # Returns
    /// A child resource bound to the same filesystem.
    #[inline]
    #[must_use]
    pub fn child(&self, name: &FsName) -> FileResource {
        let path = self.path().child(name);
        FileResource::new(self.resource.fs_arc(), path)
    }

    /// Builds a nested descendant using a validated relative path.
    ///
    /// # Parameters
    /// - `path`: Relative path that cannot escape its base.
    ///
    /// # Returns
    /// A descendant resource bound to the same filesystem.
    #[inline]
    #[must_use]
    pub fn descendant(&self, path: &RelativeFsPath) -> FileResource {
        let path = self.path().join_relative(path);
        FileResource::new(self.resource.fs_arc(), path)
    }

    /// Creates and returns one validated child directory.
    ///
    /// # Errors
    /// Returns the owning filesystem's directory creation error.
    pub fn create_child_dir(
        &self,
        name: &FsName,
        options: CreateDirOptions,
    ) -> FsResult<FileResource> {
        let child = self.child(name);
        child.create_dir(options)?;
        Ok(child)
    }

    /// Explicitly deletes the source and releases cleanup responsibility.
    ///
    /// # Errors
    /// Returns invalid-state or the provider cleanup error. A definite cleanup
    /// failure leaves this handle in [`TempResourceState::CleanupRequired`].
    pub fn cleanup(&mut self) -> FsResult<()> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            return Err(self.invalid_state(
                FsOperation::CleanupTemp,
                "temporary directory cannot be cleaned now",
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
    /// The retained source path.
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
                "temporary directory cannot be kept now",
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

    /// Publishes this temporary directory without consuming the handle.
    ///
    /// The same partial-success contract as [`crate::TempFile::persist`] is
    /// used: not-published failures remain retryable, published-source-retained
    /// failures retain cleanup responsibility, and indeterminate failures are
    /// never automatically cleaned or republished. Required atomicity is
    /// checked against capabilities before calling the provider session.
    ///
    /// # Returns
    /// Confirmed actual publication method and atomicity.
    pub fn persist(
        &mut self,
        target: &FsPath,
        options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure> {
        if self.state != TempResourceState::Owned {
            return Err(PersistFailure::new(
                self.invalid_state(
                    FsOperation::PersistTemp,
                    "temporary directory cannot be persisted now",
                ),
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

    /// Builds an invalid-state error for this temporary directory.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.path().clone())
    }
}

impl Debug for TempDir {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("TempDir")
            .field("resource", &self.resource)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) && let Err(error) = self.session.cleanup()
        {
            warn!("best-effort temporary directory cleanup failed: {error}");
        }
    }
}
