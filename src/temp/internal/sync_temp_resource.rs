// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared private lifecycle orchestration for synchronous temporary resources.

use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::path::Path;
use crate::spi::PersistRequest;
use crate::spi::SpiPersistFailure;
use crate::spi::TempResourceSpi;
use crate::temp::PersistFailure;
use crate::temp::PersistFailureState;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;
use crate::temp::internal::TempLifecycle;

/// Owns provider state and coordinates a synchronous temporary resource
/// lifecycle.
pub(crate) struct SyncTempResource {
    /// Facade used for provider validation and contextual errors.
    filesystem: FileSystem,
    /// Current provider-local resource path.
    path: Path,
    /// Provider lifecycle session held until cleanup ownership is released.
    session: Box<dyn TempResourceSpi>,
    /// Shared publication and cleanup state.
    lifecycle: TempLifecycle,
    /// Resource-specific message used for rejected lifecycle operations.
    invalid_state_message: &'static str,
}

impl SyncTempResource {
    /// Creates an owned synchronous temporary resource.
    ///
    /// # Parameters
    /// - `filesystem`: Facade that validates provider operations.
    /// - `path`: Validated provider-local resource path.
    /// - `session`: Provider session that owns the temporary resource.
    /// - `invalid_state_message`: Resource-specific invalid-state diagnostic.
    pub(crate) fn new(
        filesystem: FileSystem,
        path: Path,
        session: Box<dyn TempResourceSpi>,
        invalid_state_message: &'static str,
    ) -> Self {
        Self {
            filesystem,
            path,
            session,
            lifecycle: TempLifecycle::new(),
            invalid_state_message,
        }
    }

    /// Returns the provider-local resource path.
    pub(crate) const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the lifecycle state.
    pub(crate) const fn state(&self) -> TempResourceState {
        self.lifecycle.state()
    }

    /// Persists this temporary resource to `target`.
    ///
    /// # Parameters
    /// - `target`: Destination path validated by the owning filesystem.
    /// - `options`: Atomicity and publication requirements.
    ///
    /// # Returns
    /// The provider-confirmed publication outcome.
    ///
    /// # Errors
    /// Returns a typed failure for invalid lifecycle state, failed preflight,
    /// provider failure, or a provider contract violation.
    #[allow(clippy::result_large_err)]
    pub(crate) fn persist(&mut self, target: &Path, options: PersistOptions) -> Result<PersistOutcome, PersistFailure> {
        if self.lifecycle.state() != TempResourceState::Owned {
            return Err(PersistFailure::new(
                self.invalid_state(FsOperation::PersistTemp).with_target(target.clone()),
                self.lifecycle.failure_state(),
            )
            .with_publication_target(self.lifecycle.publication_target()));
        }
        if let Err(error) = self.filesystem.preflight_temp_persist(&self.path, target, &options) {
            return Err(PersistFailure::new(error, PersistFailureState::NotPublished));
        }
        match self.session.persist(PersistRequest::new(target, options.clone())) {
            Ok(outcome) => {
                if outcome.target() != target {
                    self.lifecycle
                        .record_failure(PersistFailureState::Indeterminate, Some(target.clone()), false);
                    return Err(PersistFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::PersistTemp,
                            "provider reported a persistence target different from the request",
                        )
                        .with_path(self.path.clone())
                        .with_target(target.clone())
                        .with_provider(self.filesystem.properties().info().provider_id()),
                        PersistFailureState::Indeterminate,
                    ));
                }
                if options.atomicity() == AtomicityRequirement::Required
                    && outcome.atomicity() != AchievedAtomicity::Atomic
                {
                    self.lifecycle.record_failure(
                        PersistFailureState::PublishedSourceIndeterminate,
                        Some(target.clone()),
                        false,
                    );
                    return Err(PersistFailure::new(
                        FsError::new(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::PersistTemp,
                            "provider reported non-atomic success for atomic-required persist",
                        )
                        .with_path(self.path.clone())
                        .with_target(target.clone())
                        .with_provider(self.filesystem.properties().info().provider_id()),
                        PersistFailureState::PublishedSourceIndeterminate,
                    )
                    .with_publication_target(self.lifecycle.publication_target()));
                }
                self.lifecycle.record_success(false, outcome.target().clone());
                Ok(outcome)
            }
            Err(failure) => Err(self.record_persist_failure(failure, target, FsOperation::PersistTemp)),
        }
    }
    /// Publishes this temporary resource to the provider-generated target.
    ///
    /// # Returns
    /// The provider-confirmed publication outcome and generated target.
    ///
    /// # Errors
    /// Returns a typed failure when ownership is unavailable, provider
    /// publication fails, or the provider returns an invalid target.
    #[allow(clippy::result_large_err)]
    pub(crate) fn keep(&mut self) -> Result<PersistOutcome, PersistFailure> {
        if let Err(error) = self.ensure_owned(FsOperation::KeepTemp) {
            return Err(PersistFailure::new(error, self.lifecycle.failure_state())
                .with_publication_target(self.lifecycle.publication_target()));
        }
        match self.session.keep() {
            Ok(outcome) => {
                if let Err(error) = self.filesystem.validate_temp_keep_target(&self.path, outcome.target()) {
                    self.lifecycle.record_failure(
                        PersistFailureState::Indeterminate,
                        Some(outcome.target().clone()),
                        true,
                    );
                    return Err(PersistFailure::new(error, PersistFailureState::Indeterminate));
                }
                self.path = outcome.target().clone();
                self.lifecycle.record_success(true, outcome.target().clone());
                Ok(outcome)
            }
            Err(failure) => Err(self.record_persist_failure(failure, &self.path.clone(), FsOperation::KeepTemp)),
        }
    }
    /// Cleans the source and releases the session responsibility.
    ///
    /// Call this method when cleanup success or failure must be observed.
    /// Unlike dropping an owned handle, this returns the provider result. If
    /// cleanup fails while the state remains `CleanupRequired`, a later drop
    /// may make another best-effort attempt.
    ///
    /// # Errors
    /// Returns an invalid-state error when cleanup is no longer legal, or the
    /// provider cleanup error when cleanup cannot be confirmed.
    pub(crate) fn cleanup(&mut self) -> FsResult<()> {
        if !matches!(
            self.lifecycle.state(),
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            return Err(self.invalid_state(FsOperation::CleanupTemp));
        }
        self.session
            .cleanup()
            .map(|()| self.lifecycle.record_cleanup_success())
            .map_err(|error| self.record_lifecycle_error(error, FsOperation::CleanupTemp))
    }
    /// Records provider-reported persistence progress in lifecycle state and
    /// error.
    ///
    /// # Parameters
    /// - `failure`: Provider-reported failure and publication facts.
    /// - `target`: Destination requested by the persistence operation.
    /// - `operation`: Persistence or keep operation being contextualized.
    ///
    /// # Returns
    /// A typed failure retaining provider state and the latest confirmed
    /// target.
    fn record_persist_failure(
        &mut self,
        failure: SpiPersistFailure,
        target: &Path,
        operation: FsOperation,
    ) -> PersistFailure {
        let (error, state) = failure.into_parts();
        let publication_target = if operation == FsOperation::KeepTemp {
            error.target().cloned()
        } else {
            Some(target.clone())
        };
        self.lifecycle
            .record_failure(state, publication_target, operation == FsOperation::KeepTemp);
        PersistFailure::new(
            error.with_operation(operation).with_missing_context(
                &self.path,
                Some(target),
                self.filesystem.properties().info().provider_id(),
            ),
            state,
        )
        .with_publication_target(self.lifecycle.publication_target())
    }
    /// Requires an owned, unpublished resource.
    ///
    /// # Parameters
    /// - `operation`: Lifecycle operation attempted by the caller.
    ///
    /// # Returns
    /// `Ok(())` while this resource owns an unpublished source; otherwise an
    /// invalid-state error.
    fn ensure_owned(&self, operation: FsOperation) -> FsResult<()> {
        if self.lifecycle.state() == TempResourceState::Owned {
            Ok(())
        } else {
            Err(self.invalid_state(operation))
        }
    }
    /// Records a cleanup or ownership-transfer error with resource context.
    ///
    /// # Parameters
    /// - `error`: Cleanup or ownership-transfer error to retain.
    /// - `operation`: Lifecycle operation associated with the error.
    ///
    /// # Returns
    /// The error enriched with path and provider context.
    fn record_lifecycle_error(&mut self, error: FsError, operation: FsOperation) -> FsError {
        self.lifecycle.record_cleanup_error(&error);
        error.with_operation(operation).with_missing_context(
            &self.path,
            None,
            self.filesystem.properties().info().provider_id(),
        )
    }
    /// Builds a contextual invalid-state error.
    ///
    /// # Parameters
    /// - `operation`: Lifecycle operation rejected by the current state.
    ///
    /// # Returns
    /// An invalid-state error carrying this resource's path and provider.
    fn invalid_state(&self, operation: FsOperation) -> FsError {
        FsError::new(FsErrorKind::InvalidState, operation, self.invalid_state_message)
            .with_path(self.path.clone())
            .with_provider(self.filesystem.properties().info().provider_id())
    }
}

impl Drop for SyncTempResource {
    /// Attempts best-effort cleanup while this resource still owns it.
    fn drop(&mut self) {
        if matches!(
            self.lifecycle.state(),
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let _ = self.session.cleanup();
        }
    }
}
