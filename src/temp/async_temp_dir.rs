// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Concrete asynchronous temporary directory handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::pin::Pin;

use crate::{
    AsyncDirectoryStream,
    AsyncFileResource,
    AsyncTempResourceSession,
    CreateDirOptions,
    FsError,
    FsErrorKind,
    FsFuture,
    FsName,
    FsOperation,
    FsPath,
    ListOptions,
    PersistFailure,
    PersistFailureState,
    PersistFuture,
    PersistOptions,
    RelativeFsPath,
    TempResourceState,
};

/// Type-erased asynchronous temporary directory with explicit lifecycle.
pub struct AsyncTempDir {
    resource: AsyncFileResource,
    session: Pin<Box<dyn AsyncTempResourceSession>>,
    state: TempResourceState,
}

impl AsyncTempDir {
    /// Creates an async temporary directory from its resource and session.
    #[inline]
    #[must_use]
    pub fn new<S>(resource: AsyncFileResource, session: S) -> Self
    where
        S: AsyncTempResourceSession + 'static,
    {
        Self {
            resource,
            session: Box::pin(session),
            state: TempResourceState::Owned,
        }
    }

    /// Returns the provider-local temporary directory path.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        self.resource.path()
    }

    /// Returns the bound asynchronous resource.
    #[inline]
    #[must_use]
    pub fn resource(&self) -> &AsyncFileResource {
        &self.resource
    }

    /// Returns the current ownership and recovery state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.state
    }

    /// Asynchronously lists the temporary directory.
    #[inline]
    pub fn list_async(
        &self,
        options: ListOptions,
    ) -> FsFuture<'_, AsyncDirectoryStream> {
        self.resource.list_async(options)
    }

    /// Builds an immediate child from a validated single name.
    #[inline]
    #[must_use]
    pub fn child(&self, name: &FsName) -> AsyncFileResource {
        AsyncFileResource::new(self.resource.fs_arc(), self.path().child(name))
    }

    /// Builds a nested descendant from a safe relative path.
    #[inline]
    #[must_use]
    pub fn descendant(&self, path: &RelativeFsPath) -> AsyncFileResource {
        AsyncFileResource::new(
            self.resource.fs_arc(),
            self.path().join_relative(path),
        )
    }

    /// Asynchronously creates and returns a validated child directory.
    pub fn create_child_dir_async(
        &self,
        name: &FsName,
        options: CreateDirOptions,
    ) -> FsFuture<'_, AsyncFileResource> {
        let child = self.child(name);
        Box::pin(async move {
            child.create_dir_async(options).await?;
            Ok(child)
        })
    }

    /// Asynchronously cleans the source and confirms provider cleanup.
    ///
    /// Once polled, dropping the returned future before completion leaves the
    /// handle indeterminate and disables automatic cancellation.
    pub fn cleanup_async(&mut self) -> FsFuture<'_, ()> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(
                FsOperation::CleanupTemp,
                "async temporary directory cannot be cleaned now",
            );
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.state = TempResourceState::Indeterminate;
            let result = self.session.as_mut().cleanup_async().await;
            match result {
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
        })
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    ///
    /// Once polled, dropping the returned future before completion leaves the
    /// handle indeterminate. A definite provider failure restores the state
    /// from which the ownership transfer was started.
    pub fn keep_async(&mut self) -> FsFuture<'_, FsPath> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(
                FsOperation::KeepTemp,
                "async temporary directory cannot be kept now",
            );
            return Box::pin(async move { Err(error) });
        }
        let path = self.path().clone();
        Box::pin(async move {
            let previous_state = self.state;
            self.state = TempResourceState::Indeterminate;
            match self.session.as_mut().keep_async().await {
                Ok(()) => {
                    self.state = TempResourceState::Kept;
                    Ok(path)
                }
                Err(error) => {
                    if error.kind() != FsErrorKind::Indeterminate {
                        self.state = previous_state;
                    }
                    Err(error)
                }
            }
        })
    }

    /// Asynchronously persists this directory without consuming its handle.
    ///
    /// Failure states and drop behavior match [`crate::AsyncTempFile`]. Async
    /// drop performs only nonblocking local cancellation; confirmed remote
    /// cleanup requires awaiting an explicit lifecycle method. Once polled,
    /// dropping the persistence future before completion leaves the handle
    /// indeterminate because source or target state may have changed.
    pub fn persist_async<'a>(
        &'a mut self,
        target: &'a FsPath,
        options: PersistOptions,
    ) -> PersistFuture<'a> {
        if self.state != TempResourceState::Owned {
            let failure = PersistFailure::new(
                self.invalid_state(
                    FsOperation::PersistTemp,
                    "async temporary directory cannot be persisted now",
                ),
                PersistFailureState::NotPublished,
            );
            return Box::pin(async move { Err(failure) });
        }
        if let Err(error) = self
            .resource
            .validate_path(target, FsOperation::PersistTemp)
        {
            let failure = PersistFailure::new(
                error
                    .with_path(self.path().clone())
                    .with_target(target.clone()),
                PersistFailureState::NotPublished,
            );
            return Box::pin(async move { Err(failure) });
        }
        if let Err(error) =
            options.validate_against(self.resource.fs().capabilities())
        {
            let failure = PersistFailure::new(
                error
                    .with_path(self.path().clone())
                    .with_target(target.clone()),
                PersistFailureState::NotPublished,
            );
            return Box::pin(async move { Err(failure) });
        }
        Box::pin(async move {
            self.state = TempResourceState::Indeterminate;
            let result =
                self.session.as_mut().persist_async(target, options).await;
            match result {
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
        })
    }

    /// Builds an invalid-state error for this handle.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message)
            .with_path(self.path().clone())
    }
}

impl Debug for AsyncTempDir {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncTempDir")
            .field("resource", &self.resource)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for AsyncTempDir {
    fn drop(&mut self) {
        if matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            self.session.as_mut().cancel_on_drop();
        }
    }
}
