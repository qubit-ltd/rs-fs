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

use crate::AsyncFileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::path::Path;
use crate::path::PathComponent;
use crate::spi::AsyncTempResourceSpi;
use crate::spi::PersistRequest;
use crate::spi::SpiFuture;
use crate::temp::PersistFailure;
use crate::temp::PersistFailureState;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;
use crate::temp::internal::TempLifecycle;

/// A facade-owned asynchronous temporary file.
///
/// # Examples
///
/// This example uses an isolated in-memory provider fixture.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
/// # poll_support::ready(async {
/// use qubit_fs::temp::TempOptions;
/// use qubit_fs::temp::TempResourceState;
///
/// let mut temporary = filesystem.create_temp_file(TempOptions::default()).await?;
/// temporary.cleanup().await?;
/// assert_eq!(TempResourceState::Cleaned, temporary.state());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
pub struct AsyncTempFile {
    /// Facade that owns validation and persistence policy.
    file_system: AsyncFileSystem,
    /// Provider-local temporary path.
    path: Path,
    /// Pinned provider lifecycle session.
    session: Pin<Box<dyn AsyncTempResourceSpi>>,
    /// Current cleanup and publication lifecycle state.
    lifecycle: TempLifecycle,
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
            lifecycle: TempLifecycle::new(),
            resource_name,
        }
    }

    /// Returns the provider-local temporary path.
    ///
    /// # Returns
    /// The validated path supplied by the provider.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the current ownership lifecycle state.
    ///
    /// # Returns
    /// The handle's current cleanup and publication state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.lifecycle.state()
    }

    /// Returns one lexically safe child path.
    #[inline]
    #[must_use]
    pub fn child(&self, component: &PathComponent) -> Path {
        self.path.child(component)
    }

    /// Returns one lexically safe descendant path.
    #[inline]
    #[must_use]
    pub fn descendant(&self, relative: &crate::path::RelativePath) -> Path {
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
        self.lifecycle("cannot be cleaned now", FsOperation::CleanupTemp, |session| {
            session.cleanup()
        })
    }

    /// Asynchronously publishes this temporary resource to a generated target.
    ///
    /// # Returns
    /// A future resolving to the provider's confirmed publication outcome.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when the resource is no longer owned,
    /// or to the provider ownership-transfer failure. An invalid-state failure
    /// retains any previously confirmed publication target and recovery state.
    #[inline]
    pub fn keep(&mut self) -> SpiFuture<'_, Result<PersistOutcome, PersistFailure>> {
        if self.lifecycle.state() != TempResourceState::Owned {
            let error = self.invalid_state(FsOperation::KeepTemp, "cannot be kept now");
            return Box::pin(async move {
                Err(PersistFailure::new(error, self.lifecycle.failure_state())
                    .with_publication_target(self.lifecycle.publication_target()))
            });
        }
        Box::pin(async move {
            self.lifecycle.begin_pending();
            match self.session.as_mut().keep().await {
                Ok(outcome) => {
                    if let Err(error) = self.file_system.validate_temp_keep_target(&self.path, outcome.target()) {
                        return Err(PersistFailure::new(error, PersistFailureState::Indeterminate));
                    }
                    self.path = outcome.target().clone();
                    self.lifecycle.record_success(true, outcome.target().clone());
                    Ok(outcome)
                }
                Err(failure) => {
                    let (error, state) = failure.into_parts();
                    self.lifecycle.record_failure(state, error.target().cloned(), true);
                    let target = self.path.clone();
                    Err(PersistFailure::new(
                        error.with_operation(FsOperation::KeepTemp).with_missing_context(
                            &self.path,
                            Some(&target),
                            self.file_system.properties().info().provider_id(),
                        ),
                        state,
                    )
                    .with_publication_target(self.lifecycle.publication_target()))
                }
            }
        })
    }

    /// Asynchronously persists this resource to a validated destination.
    ///
    /// # Parameters
    /// - `target`: Validated destination path.
    /// - `options`: Persistence atomicity and publication requirements.
    /// - `options`: Persistence atomicity and publication requirements.
    ///
    /// # Returns
    /// A future resolving to the confirmed persistence outcome.
    ///
    /// # Errors
    /// Resolves to a typed failure for invalid lifecycle state, failed local
    /// preflight, provider failure, or provider contract violation. Rejected
    /// repeated calls preserve the previously confirmed publication facts.
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        if self.lifecycle.state() != TempResourceState::Owned {
            let error = self.invalid_state(FsOperation::PersistTemp, "cannot be persisted now");
            return Box::pin(async move {
                Err(PersistFailure::new(error, self.lifecycle.failure_state())
                    .with_publication_target(self.lifecycle.publication_target()))
            });
        }
        if let Err(error) = self.file_system.preflight_temp_persist(&self.path, target, &options) {
            return Box::pin(async move { Err(PersistFailure::new(error, PersistFailureState::NotPublished)) });
        }
        Box::pin(async move {
            self.lifecycle.begin_pending();
            let atomicity = options.atomicity();
            let result = self
                .session
                .as_mut()
                .persist(PersistRequest::new(target, options))
                .await;
            match &result {
                Ok(outcome)
                    if outcome.target() == target
                        && !(atomicity == AtomicityRequirement::Required
                            && outcome.atomicity() != AchievedAtomicity::Atomic) =>
                {
                    self.lifecycle.record_success(false, outcome.target().clone());
                }
                Ok(outcome) if outcome.target() != target => {
                    self.lifecycle
                        .record_failure(PersistFailureState::Indeterminate, Some(target.clone()), false);
                }
                Ok(_) => self.lifecycle.record_failure(
                    PersistFailureState::PublishedSourceRetained,
                    Some(target.clone()),
                    false,
                ),
                Err(failure) => self
                    .lifecycle
                    .record_failure(failure.state(), Some(target.clone()), false),
            }
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
                    )
                    .with_publication_target(self.lifecycle.publication_target()))
                }
                Err(failure) => {
                    let (error, state) = failure.into_parts();
                    Err(PersistFailure::new(self.contextual_persist_error(error, target), state)
                        .with_publication_target(self.lifecycle.publication_target()))
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
        F: FnOnce(Pin<&'a mut dyn AsyncTempResourceSpi>) -> SpiFuture<'a, FsResult<()>> + Send + 'a,
    {
        if !matches!(
            self.lifecycle.state(),
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let error = self.invalid_state(operation, action);
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            let previous_lifecycle = self.lifecycle.clone();
            self.lifecycle.begin_pending();
            let result = call(self.session.as_mut()).await;
            self.lifecycle = previous_lifecycle;
            match &result {
                Ok(()) => self.lifecycle.record_cleanup_success(),
                Err(error) => self.lifecycle.record_cleanup_error(error),
            }
            result.map_err(|error| {
                error.with_operation(operation).with_missing_context(
                    &self.path,
                    None,
                    self.file_system.properties().info().provider_id(),
                )
            })
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
        FsError::new(FsErrorKind::InvalidState, operation, &message).with_path(self.path.clone())
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
    fn contextual_persist_error(&self, error: FsError, target: &Path) -> FsError {
        error.with_operation(FsOperation::PersistTemp).with_missing_context(
            &self.path,
            Some(target),
            self.file_system.properties().info().provider_id(),
        )
    }
}

impl Drop for AsyncTempFile {
    fn drop(&mut self) {
        if matches!(
            self.lifecycle.state(),
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            self.session.as_mut().cancel_on_drop();
        }
    }
}
