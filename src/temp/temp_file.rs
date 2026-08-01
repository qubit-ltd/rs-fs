// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-backed temporary file lifecycle handle.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};

use crate::spi::{
    PersistRequest,
    SpiPersistFailure,
    TempResourceSpi,
};
use crate::{
    AchievedAtomicity,
    AtomicityRequirement,
    FileSystem,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    Path,
    PersistFailure,
    PersistFailureState,
    PersistOptions,
    PersistOutcome,
    TempResourceState,
};

/// Temporary file retaining the provider session until its lifecycle completes.
pub struct TempFile {
    /// Facade that owns validation and persistence policy.
    filesystem: FileSystem,
    /// Provider-local temporary file path.
    path: Path,
    /// Provider lifecycle session.
    session: Box<dyn TempResourceSpi>,
    /// Current cleanup and publication lifecycle state.
    state: TempResourceState,
}

impl TempFile {
    /// Creates the facade handle from validated provider parts.
    pub(crate) fn new(
        filesystem: FileSystem,
        path: Path,
        session: Box<dyn TempResourceSpi>,
    ) -> Self {
        Self {
            filesystem,
            path,
            session,
            state: TempResourceState::Owned,
        }
    }
    /// Returns the temporary logical path.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the resource lifecycle state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.state
    }
    /// Persists this temporary file to `target`.
    pub fn persist(
        &mut self,
        target: &Path,
        options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure> {
        if self.state != TempResourceState::Owned {
            return Err(PersistFailure::new(
                self.invalid_state(FsOperation::PersistTemp),
                PersistFailureState::NotPublished,
            ));
        }
        if let Err(error) = self
            .filesystem
            .preflight_temp_persist(&self.path, target, &options)
        {
            return Err(PersistFailure::new(
                error,
                PersistFailureState::NotPublished,
            ));
        }
        match self
            .session
            .persist(PersistRequest::new(target, options.clone()))
        {
            Ok(outcome) => {
                if outcome.target() != target {
                    self.state = TempResourceState::Indeterminate;
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
                    self.state = TempResourceState::CleanupRequired;
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
                self.state = TempResourceState::Persisted;
                Ok(outcome)
            }
            Err(failure) => Err(self.record_persist_failure(failure)),
        }
    }
    /// Releases automatic cleanup responsibility to the caller.
    pub fn keep(&mut self) -> FsResult<()> {
        self.ensure_owned(FsOperation::KeepTemp)?;
        let previous_state = self.state;
        self.session
            .keep()
            .map(|()| self.state = TempResourceState::Kept)
            .inspect_err(|error| {
                if error.kind() == FsErrorKind::Indeterminate {
                    self.state = TempResourceState::Indeterminate;
                } else {
                    self.state = previous_state;
                }
            })
    }
    /// Cleans the source and releases the session responsibility.
    pub fn cleanup(&mut self) -> FsResult<()> {
        if !matches!(
            self.state,
            TempResourceState::Owned | TempResourceState::CleanupRequired
        ) {
            return Err(self.invalid_state(FsOperation::CleanupTemp));
        }
        self.session
            .cleanup()
            .map(|()| self.state = TempResourceState::Cleaned)
            .map_err(|error| self.record_cleanup_error(error))
    }
    /// Records provider partial persistence facts in facade state and error.
    fn record_persist_failure(
        &mut self,
        failure: SpiPersistFailure,
    ) -> PersistFailure {
        let (error, state) = failure.into_parts();
        self.state = match state {
            PersistFailureState::NotPublished => TempResourceState::Owned,
            PersistFailureState::PublishedSourceRetained => {
                TempResourceState::CleanupRequired
            }
            PersistFailureState::Indeterminate => {
                TempResourceState::Indeterminate
            }
        };
        PersistFailure::new(error, state)
    }
    /// Requires an owned, unpublished source.
    fn ensure_owned(&self, operation: FsOperation) -> FsResult<()> {
        if self.state == TempResourceState::Owned {
            Ok(())
        } else {
            Err(self.invalid_state(operation))
        }
    }
    /// Records a cleanup or ownership-transfer error.
    fn record_cleanup_error(&mut self, error: FsError) -> FsError {
        if error.kind() == FsErrorKind::Indeterminate {
            self.state = TempResourceState::Indeterminate;
        } else {
            self.state = TempResourceState::CleanupRequired;
        }
        error
    }
    /// Builds a contextual invalid-state error.
    fn invalid_state(&self, operation: FsOperation) -> FsError {
        FsError::new(
            FsErrorKind::InvalidState,
            operation,
            "temporary file cannot perform this lifecycle operation",
        )
        .with_path(self.path.clone())
    }
}

impl Debug for TempFile {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("TempFile")
            .field("path", &self.path)
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
