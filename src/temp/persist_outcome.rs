// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Successful temporary resource persistence outcome.

use super::PersistCleanupState;
use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::PublicationMethod;
use crate::metadata::UserMetadata;
use crate::path::Path;

/// Confirmed result of publishing a temporary source to its final target.
#[derive(Clone, Debug, PartialEq)]
pub struct PersistOutcome {
    /// Final provider-local target path.
    target: Path,
    /// Atomicity actually achieved by publication.
    atomicity: AchievedAtomicity,
    /// Concrete publication method used.
    method: PublicationMethod,
    /// Provider-native non-sensitive diagnostics.
    diagnostics: NonSensitiveMetadata,
    /// Cleanup state of the private temporary container.
    cleanup_state: PersistCleanupState,
}

impl PersistOutcome {
    /// Creates a confirmed persistence outcome.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the target.
    ///
    /// # Returns
    /// An outcome without provider diagnostics.
    #[inline]
    #[must_use]
    pub fn new(
        target: Path,
        atomicity: AchievedAtomicity,
        method: PublicationMethod,
    ) -> Self {
        Self {
            target,
            atomicity,
            method,
            diagnostics: NonSensitiveMetadata::new(),
            cleanup_state: PersistCleanupState::Complete,
        }
    }

    /// Returns the final provider-local target path.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the atomicity actually achieved by publication.
    #[inline(always)]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }

    /// Returns the concrete publication method used.
    #[inline(always)]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns provider-native non-sensitive diagnostics.
    #[inline(always)]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }

    /// Returns the state of the private temporary container after publication.
    #[inline(always)]
    pub const fn cleanup_state(&self) -> PersistCleanupState {
        self.cleanup_state
    }

    /// Replaces the cleanup state reported by the provider.
    #[inline]
    #[must_use]
    pub fn with_cleanup_state(
        mut self,
        cleanup_state: PersistCleanupState,
    ) -> Self {
        self.cleanup_state = cleanup_state;
        self
    }

    /// Replaces provider-native diagnostics that have already passed key
    /// validation.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }
}
