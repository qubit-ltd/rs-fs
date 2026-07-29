// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Runtime-neutral asynchronous temporary-resource facade handles.

use std::pin::Pin;

use crate::spi::{AsyncTempResourceSpi, PersistRequest, SpiFuture};
use crate::{
    AchievedAtomicity, AsyncFileSystem, AtomicityRequirement, FsError, FsErrorKind, FsOperation,
    FsResult, Path, PersistFailure, PersistFailureState, PersistOptions, PersistOutcome,
    TempResourceState,
};

/// A facade-owned asynchronous temporary file.
pub struct AsyncTempFile {
    file_system: AsyncFileSystem,
    path: Path,
    session: Pin<Box<dyn AsyncTempResourceSpi>>,
    state: TempResourceState,
}

impl AsyncTempFile {
    /// Binds a validated provider temporary session to its owning facade.
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        path: Path,
        session: Box<dyn AsyncTempResourceSpi>,
    ) -> Self {
        Self {
            file_system,
            path,
            session: Box::into_pin(session),
            state: TempResourceState::Owned,
        }
    }

    /// Returns the provider-local temporary path.
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the current ownership lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.state
    }

    /// Asynchronously confirms cleanup of this temporary resource.
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.lifecycle(
            "temporary file cannot be cleaned now",
            FsOperation::CleanupTemp,
            |session| session.cleanup(),
        )
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    pub fn keep(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.lifecycle(
            "temporary file cannot be kept now",
            FsOperation::KeepTemp,
            |session| session.keep(),
        )
    }

    /// Asynchronously persists this resource to a validated destination.
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        if self.state != TempResourceState::Owned {
            let error = self.invalid_state(
                FsOperation::PersistTemp,
                "temporary file cannot be persisted now",
            );
            return Box::pin(async move {
                Err(PersistFailure::new(
                    error,
                    PersistFailureState::NotPublished,
                ))
            });
        }
        if let Err(error) = self
            .file_system
            .preflight_temp_persist(&self.path, target, &options)
        {
            return Box::pin(async move {
                Err(PersistFailure::new(
                    error,
                    PersistFailureState::NotPublished,
                ))
            });
        }
        Box::pin(async move {
            self.state = TempResourceState::Indeterminate;
            let atomicity = options.atomicity;
            let result = self
                .session
                .as_mut()
                .persist(PersistRequest::new(target, options))
                .await;
            self.state = match &result {
                Ok(outcome)
                    if atomicity == AtomicityRequirement::Required
                        && outcome.atomicity != AchievedAtomicity::Atomic =>
                {
                    TempResourceState::CleanupRequired
                }
                Ok(_) => TempResourceState::Persisted,
                Err(failure) => match failure.state() {
                    PersistFailureState::NotPublished => TempResourceState::Owned,
                    PersistFailureState::PublishedSourceRetained => {
                        TempResourceState::CleanupRequired
                    }
                    PersistFailureState::Indeterminate => TempResourceState::Indeterminate,
                },
            };
            match result {
                Ok(outcome)
                    if atomicity == AtomicityRequirement::Required
                        && outcome.atomicity != AchievedAtomicity::Atomic =>
                {
                    Err(PersistFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::PersistTemp,
                            "provider reported non-atomic success for atomic-required persist",
                        )
                        .with_path(self.path.clone())
                        .with_target(target.clone()),
                        PersistFailureState::PublishedSourceRetained,
                    ))
                }
                Err(failure) => {
                    let (error, state) = failure.into_parts();
                    Err(PersistFailure::new(
                        self.contextual_persist_error(error, target),
                        state,
                    ))
                }
                Ok(outcome) => Ok(outcome),
            }
        })
    }

    /// Runs one lifecycle operation while retaining an indeterminate
    /// cancellation state.
    fn lifecycle<'a, F>(
        &'a mut self,
        message: &'static str,
        operation: FsOperation,
        call: F,
    ) -> SpiFuture<'a, FsResult<()>>
    where
        F: FnOnce(Pin<&'a mut dyn AsyncTempResourceSpi>) -> SpiFuture<'a, FsResult<()>> + Send + 'a,
    {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(operation, message);
            return Box::pin(async move { Err(error) });
        }
        let previous_state = self.state;
        Box::pin(async move {
            self.state = TempResourceState::Indeterminate;
            let result = call(self.session.as_mut()).await;
            self.state = match (operation, &result) {
                (FsOperation::CleanupTemp, Ok(())) => TempResourceState::Cleaned,
                (FsOperation::KeepTemp, Ok(())) => TempResourceState::Kept,
                (_, Err(error)) if error.kind() == FsErrorKind::Indeterminate => {
                    TempResourceState::Indeterminate
                }
                (FsOperation::KeepTemp, Err(_)) => previous_state,
                _ => TempResourceState::CleanupRequired,
            };
            result
        })
    }

    /// Builds an invalid-state error for this handle.
    fn invalid_state(&self, operation: FsOperation, message: &str) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, message).with_path(self.path.clone())
    }

    /// Adds only missing facade facts to a provider persistence error.
    fn contextual_persist_error(&self, error: FsError, target: &Path) -> FsError {
        error
            .with_operation(FsOperation::PersistTemp)
            .with_missing_context(
                &self.path,
                Some(target),
                self.file_system.properties().info().provider_id(),
            )
    }
}

/// A facade-owned asynchronous temporary directory.
pub struct AsyncTempDirectory(AsyncTempFile);

impl AsyncTempDirectory {
    /// Binds a validated provider temporary-directory session to its facade.
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        path: Path,
        session: Box<dyn AsyncTempResourceSpi>,
    ) -> Self {
        Self(AsyncTempFile::new(file_system, path, session))
    }

    /// Returns the provider-local temporary path.
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.0.path()
    }

    /// Returns the current ownership lifecycle state.
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.0.state()
    }

    /// Asynchronously confirms cleanup of this temporary directory.
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.cleanup()
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    pub fn keep(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.keep()
    }

    /// Asynchronously persists this directory to a validated destination.
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        self.0.persist(target, options)
    }
}
