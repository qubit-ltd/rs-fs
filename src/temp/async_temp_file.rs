// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Runtime-neutral asynchronous temporary-file facade handle.

use std::pin::Pin;

use crate::spi::{
    AsyncTempResourceSpi,
    PersistRequest,
    SpiFuture,
};
use crate::{
    AchievedAtomicity,
    AsyncFileSystem,
    AtomicityRequirement,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    Path,
    PathComponent,
    PersistFailure,
    PersistFailureState,
    PersistOptions,
    PersistOutcome,
    TempResourceState,
};

/// A facade-owned asynchronous temporary file.
pub struct AsyncTempFile {
    /// Facade that owns validation and persistence policy.
    file_system: AsyncFileSystem,
    /// Provider-local temporary path.
    path: Path,
    /// Pinned provider lifecycle session.
    session: Pin<Box<dyn AsyncTempResourceSpi>>,
    /// Current cleanup and publication lifecycle state.
    state: TempResourceState,
    /// Human-readable resource kind used in lifecycle diagnostics.
    resource_name: &'static str,
}

impl AsyncTempFile {
    /// Binds a validated provider temporary session to its owning facade.
    ///
    /// # Parameters
    /// - `file_system`: Facade that owns validation and persistence policy.
    /// - `path`: Validated provider-local temporary path.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// An owned asynchronous temporary-file handle.
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        path: Path,
        session: Box<dyn AsyncTempResourceSpi>,
        resource_name: &'static str,
    ) -> Self {
        Self {
            file_system,
            path,
            session: Box::into_pin(session),
            state: TempResourceState::Owned,
            resource_name,
        }
    }

    /// Returns the provider-local temporary path.
    ///
    /// # Returns
    /// The validated path supplied by the provider.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the current ownership lifecycle state.
    ///
    /// # Returns
    /// The handle's current cleanup and publication state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.state
    }

    /// Returns one lexically safe child path.
    #[inline(always)]
    #[must_use]
    pub fn child(&self, component: &PathComponent) -> Path {
        self.path.child(component)
    }

    /// Returns one lexically safe descendant path.
    #[inline(always)]
    #[must_use]
    pub fn descendant(&self, relative: &crate::RelativePath) -> Path {
        self.path.join(relative)
    }

    /// Asynchronously confirms cleanup of this temporary resource.
    ///
    /// # Returns
    /// A future resolving after provider cleanup is confirmed.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when cleanup is no longer legal, or
    /// to the provider cleanup failure.
    #[inline]
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.lifecycle(
            "cannot be cleaned now",
            FsOperation::CleanupTemp,
            |session| session.cleanup(),
        )
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    ///
    /// # Returns
    /// A future resolving after the provider confirms ownership transfer.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when the resource is no longer owned, or
    /// to the provider ownership-transfer failure.
    #[inline]
    pub fn keep(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.lifecycle(
            "cannot be kept now",
            FsOperation::KeepTemp,
            |session| session.keep(),
        )
    }

    /// Asynchronously persists this resource to a validated destination.
    ///
    /// # Parameters
    /// - `target`: Validated destination path.
    /// - `options`: Persistence atomicity and publication requirements.
    ///
    /// # Returns
    /// A future resolving to the confirmed persistence outcome.
    ///
    /// # Errors
    /// Resolves to a typed failure for invalid lifecycle state, failed local
    /// preflight, provider failure, or provider contract violation.
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        if self.state != TempResourceState::Owned {
            let error = self.invalid_state(
                FsOperation::PersistTemp,
                "cannot be persisted now",
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
            let atomicity = options.atomicity();
            let result = self
                .session
                .as_mut()
                .persist(PersistRequest::new(target, options))
                .await;
            self.state = match &result {
                Ok(outcome) if outcome.target() != target => {
                    TempResourceState::Indeterminate
                }
                Ok(outcome)
                    if atomicity == AtomicityRequirement::Required
                        && outcome.atomicity() != AchievedAtomicity::Atomic =>
                {
                    TempResourceState::CleanupRequired
                }
                Ok(_) => TempResourceState::Persisted,
                Err(failure) => match failure.state() {
                    PersistFailureState::NotPublished => {
                        TempResourceState::Owned
                    }
                    PersistFailureState::PublishedSourceRetained => {
                        TempResourceState::CleanupRequired
                    }
                    PersistFailureState::Indeterminate => {
                        TempResourceState::Indeterminate
                    }
                },
            };
            match result {
                Ok(outcome) if outcome.target() != target => Err(PersistFailure::new(
                    FsError::new(
                        FsErrorKind::ProviderContractViolation,
                        FsOperation::PersistTemp,
                        "provider reported a persistence target different from the request",
                    )
                    .with_path(self.path.clone())
                    .with_target(target.clone()),
                    PersistFailureState::Indeterminate,
                )),
                Ok(outcome)
                    if atomicity == AtomicityRequirement::Required
                        && outcome.atomicity() != AchievedAtomicity::Atomic =>
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
    ///
    /// # Type Parameters
    /// - `F`: One-shot provider lifecycle operation.
    ///
    /// # Parameters
    /// - `action`: Resource-specific action text used when the lifecycle state
    ///   rejects the operation.
    /// - `operation`: Filesystem operation recorded in generated errors.
    /// - `call`: Provider operation invoked after local state validation.
    ///
    /// # Returns
    /// A future resolving to the provider lifecycle result.
    ///
    /// # Errors
    /// Resolves to an invalid-state error or the provider lifecycle failure.
    fn lifecycle<'a, F>(
        &'a mut self,
        action: &'static str,
        operation: FsOperation,
        call: F,
    ) -> SpiFuture<'a, FsResult<()>>
    where
        F: FnOnce(
                Pin<&'a mut dyn AsyncTempResourceSpi>,
            ) -> SpiFuture<'a, FsResult<()>>
            + Send
            + 'a,
    {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(operation, action);
            return Box::pin(async move { Err(error) });
        }
        let previous_state = self.state;
        Box::pin(async move {
            self.state = TempResourceState::Indeterminate;
            let result = call(self.session.as_mut()).await;
            self.state = match (operation, &result) {
                (FsOperation::CleanupTemp, Ok(())) => {
                    TempResourceState::Cleaned
                }
                (FsOperation::KeepTemp, Ok(())) => TempResourceState::Kept,
                (_, Err(error))
                    if error.kind() == FsErrorKind::Indeterminate =>
                {
                    TempResourceState::Indeterminate
                }
                (FsOperation::KeepTemp, Err(_)) => previous_state,
                _ => TempResourceState::CleanupRequired,
            };
            result
        })
    }

    /// Builds an invalid-state error for this handle.
    ///
    /// # Parameters
    /// - `operation`: Rejected lifecycle operation.
    /// - `action`: Stable action text describing the rejected operation.
    ///
    /// # Returns
    /// A contextual invalid-state error containing the temporary path.
    fn invalid_state(&self, operation: FsOperation, action: &str) -> FsError {
        let message = format!("{} {}", self.resource_name, action);
        FsError::new(FsErrorKind::InvalidState, operation, &message)
            .with_path(self.path.clone())
    }

    /// Adds only missing facade facts to a provider persistence error.
    ///
    /// # Parameters
    /// - `error`: Provider persistence error.
    /// - `target`: Requested persistence target.
    ///
    /// # Returns
    /// The error enriched with missing operation, path, target, and provider
    /// context.
    fn contextual_persist_error(
        &self,
        error: FsError,
        target: &Path,
    ) -> FsError {
        error
            .with_operation(FsOperation::PersistTemp)
            .with_missing_context(
                &self.path,
                Some(target),
                self.file_system.properties().info().provider_id(),
            )
    }
}
