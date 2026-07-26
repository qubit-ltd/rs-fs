// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Concrete asynchronous temporary file handle.

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::pin::Pin;

use crate::{
    AsyncFileReader, AsyncFileResource, AsyncFileWriter, AsyncTempResourceSession, FsError,
    FsErrorKind, FsFuture, FsOperation, FsPath, PersistFailure, PersistFailureState, PersistFuture,
    PersistOptions, ReadOptions, TempResourceState, WriteOptions,
};

/// Type-erased async temporary file with explicit remote lifecycle methods.
pub struct AsyncTempFile {
    resource: AsyncFileResource,
    session: Pin<Box<dyn AsyncTempResourceSession>>,
    state: TempResourceState,
}

impl AsyncTempFile {
    /// Creates an async temporary file from its resource and provider session.
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

    /// Returns the temporary provider-local path.
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

    /// Asynchronously opens the temporary file for reading.
    #[inline]
    pub fn open_reader_async(&self, options: ReadOptions) -> FsFuture<'_, AsyncFileReader> {
        self.resource.open_reader_async(options)
    }

    /// Asynchronously opens the temporary file for writing.
    #[inline]
    pub fn open_writer_async(&self, options: WriteOptions) -> FsFuture<'_, AsyncFileWriter> {
        self.resource.open_writer_async(options)
    }

    /// Asynchronously cleans the source and confirms provider cleanup.
    ///
    /// Unlike drop, this method may await remote work and reports its result.
    /// Once polled, dropping the returned future before completion leaves the
    /// handle indeterminate and disables automatic cancellation.
    pub fn cleanup_async(&mut self) -> FsFuture<'_, ()> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(
                FsOperation::CleanupTemp,
                "async temporary file cannot be cleaned now",
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

    /// Asynchronously releases cleanup responsibility and returns the source.
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
                "async temporary file cannot be kept now",
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

    /// Asynchronously publishes this source without consuming the handle.
    ///
    /// Not-published failures remain retryable, published-source-retained
    /// failures keep explicit cleanup responsibility, and indeterminate
    /// failures disable automatic cancellation or retry. Required atomicity is
    /// checked before the provider session is polled.
    ///
    /// Async drop never blocks, creates a runtime, or waits for remote cleanup;
    /// it can only invoke [`AsyncTempResourceSession::cancel_on_drop`]. Callers
    /// must explicitly await cleanup or persistence when confirmation matters.
    /// Once polled, dropping the persistence future before completion leaves
    /// the handle indeterminate because source or target state may have
    /// changed.
    pub fn persist_async<'a>(
        &'a mut self,
        target: &'a FsPath,
        options: PersistOptions,
    ) -> PersistFuture<'a> {
        if self.state != TempResourceState::Owned {
            let failure = PersistFailure::new(
                self.invalid_state(
                    FsOperation::PersistTemp,
                    "async temporary file cannot be persisted now",
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
        if let Err(error) = options.validate_against(self.resource.fs().capabilities()) {
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
            let result = self.session.as_mut().persist_async(target, options).await;
            match result {
                Ok(outcome) => {
                    self.state = TempResourceState::Persisted;
                    Ok(outcome)
                }
                Err(failure) => {
                    self.state = match failure.state() {
                        PersistFailureState::NotPublished => TempResourceState::Owned,
                        PersistFailureState::PublishedSourceRetained => {
                            TempResourceState::CleanupRequired
                        }
                        PersistFailureState::Indeterminate => TempResourceState::Indeterminate,
                    };
                    Err(failure)
                }
            }
        })
    }

    /// Builds an invalid-state error for this handle.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message).with_path(self.path().clone())
    }
}

impl Debug for AsyncTempFile {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncTempFile")
            .field("resource", &self.resource)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Drop for AsyncTempFile {
    fn drop(&mut self) {
        if matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            self.session.as_mut().cancel_on_drop();
        }
    }
}
