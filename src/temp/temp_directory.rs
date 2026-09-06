// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-backed temporary directory lifecycle handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::path::Path;
use crate::path::PathComponent;
use crate::path::RelativePath;
use crate::spi::PersistRequest;
use crate::spi::SpiPersistFailure;
use crate::spi::TempResourceSpi;
use crate::temp::PersistFailure;
use crate::temp::PersistFailureState;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;
use crate::temp::internal::TempLifecycle;

/// Temporary directory retaining the provider session until lifecycle
/// completion.
pub struct TempDirectory {
    /// Facade that owns validation and persistence policy.
    filesystem: FileSystem,
    /// Provider-local temporary directory path.
    path: Path,
    /// Provider lifecycle session.
    session: Box<dyn TempResourceSpi>,
    /// Current cleanup and publication lifecycle state.
    lifecycle: TempLifecycle,
}
impl TempDirectory {
    /// Creates the facade handle from validated provider parts.
    pub(crate) fn new(filesystem: FileSystem, path: Path, session: Box<dyn TempResourceSpi>) -> Self {
        Self {
            filesystem,
            path,
            session,
            lifecycle: TempLifecycle::new(),
        }
    }
    /// Returns the logical temporary directory path.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the resource lifecycle state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.lifecycle.state()
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
    pub fn descendant(&self, relative: &RelativePath) -> Path {
        self.path.join(relative)
    }
    /// Persists this directory.
    #[allow(clippy::result_large_err)]
    pub fn persist(&mut self, target: &Path, options: PersistOptions) -> Result<PersistOutcome, PersistFailure> {
        if self.lifecycle.state() != TempResourceState::Owned {
            return Err(PersistFailure::new(
                self.invalid_state(FsOperation::PersistTemp),
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
                        .with_target(target.clone()),
                        PersistFailureState::Indeterminate,
                    ));
                }
                if options.atomicity() == AtomicityRequirement::Required
                    && outcome.atomicity() != AchievedAtomicity::Atomic
                {
                    self.lifecycle.record_failure(
                        PersistFailureState::PublishedSourceRetained,
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
                        .with_target(target.clone()),
                        PersistFailureState::PublishedSourceRetained,
                    ));
                }
                self.lifecycle.record_success(false, outcome.target().clone());
                Ok(outcome)
            }
            Err(failure) => Err(self.record_persist_failure(failure, target, FsOperation::PersistTemp)),
        }
    }
    /// Publishes this temporary directory to the provider-generated target.
    #[allow(clippy::result_large_err)]
    pub fn keep(&mut self) -> Result<PersistOutcome, PersistFailure> {
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
    /// Cleans the temporary directory.
    pub fn cleanup(&mut self) -> FsResult<()> {
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
    /// Records provider partial persistence facts.
    fn record_persist_failure(
        &mut self,
        failure: SpiPersistFailure,
        target: &Path,
        operation: FsOperation,
    ) -> PersistFailure {
        let (error, state) = failure.into_parts();
        self.lifecycle.record_failure(state, Some(target.clone()), false);
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
    /// Requires ownership of an unpublished source.
    fn ensure_owned(&self, operation: FsOperation) -> FsResult<()> {
        if self.lifecycle.state() == TempResourceState::Owned {
            Ok(())
        } else {
            Err(self.invalid_state(operation))
        }
    }
    /// Records lifecycle error state with resource context.
    fn record_lifecycle_error(&mut self, error: FsError, operation: FsOperation) -> FsError {
        self.lifecycle.record_cleanup_error(&error);
        error.with_operation(operation).with_missing_context(
            &self.path,
            None,
            self.filesystem.properties().info().provider_id(),
        )
    }
    /// Builds a contextual invalid-state error.
    fn invalid_state(&self, operation: FsOperation) -> FsError {
        FsError::new(
            FsErrorKind::InvalidState,
            operation,
            "temporary directory cannot perform this lifecycle operation",
        )
        .with_path(self.path.clone())
    }
}
impl Debug for TempDirectory {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("TempDirectory")
            .field("path", &self.path)
            .field("state", &self.lifecycle.state())
            .finish_non_exhaustive()
    }
}
impl Drop for TempDirectory {
    fn drop(&mut self) {
        if matches!(
            self.lifecycle.state(),
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            let _ = self.session.cleanup();
        }
    }
}
