// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Successful temporary resource persistence outcome.

use super::PersistCleanupState;
use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::PublicationMethod;
use crate::metadata::UserMetadata;
use crate::path::Path;

/// Confirmed result of publishing a temporary source to its final target.
///
/// A successful outcome remains authoritative even when cleanup state is
/// [`PersistCleanupState::ResidualTemporaryContainer`]. Do not publish again or
/// clean up the published target in that case. The portable API does not expose
/// a private-container locator or retry operation; providers must report the
/// residual state without promising automatic background cleanup. The outcome
/// retains validated non-sensitive provider diagnostics for the provider's
/// existing operational mechanisms. Once a handle is `Persisted` or `Kept`, it
/// cannot be cleaned up, persisted, or kept again.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{AchievedAtomicity, PublicationMethod};
/// use qubit_fs::path::Path;
/// use qubit_fs::temp::PersistOutcome;
///
/// let outcome = PersistOutcome::new(
///     Path::parse("/published")?,
///     AchievedAtomicity::Atomic,
///     PublicationMethod::Direct,
/// );
/// assert_eq!("/published", outcome.target().as_str());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
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
    pub fn new(target: Path, atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            target,
            atomicity,
            method,
            diagnostics: NonSensitiveMetadata::new(),
            cleanup_state: PersistCleanupState::Complete,
        }
    }

    /// Returns the final provider-local target path.
    #[inline]
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the atomicity actually achieved by publication.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }

    /// Returns the concrete publication method used.
    #[inline]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns provider-native non-sensitive diagnostics already validated
    /// when the outcome was constructed.
    ///
    /// Providers may use these diagnostics with their existing operational
    /// mechanisms to investigate residual private containers; this accessor
    /// does not expose a portable cleanup or retry operation.
    #[inline]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }

    /// Returns the private temporary-container cleanup result after
    /// publication.
    ///
    /// `Complete` means the private container was removed. A
    /// `ResidualTemporaryContainer` means publication succeeded but the private
    /// container remains; it does not mean publication failed or that the
    /// handle still owns the published source.
    #[inline]
    pub const fn cleanup_state(&self) -> PersistCleanupState {
        self.cleanup_state
    }

    /// Replaces the cleanup state reported by the provider.
    #[inline]
    #[must_use]
    pub fn with_cleanup_state(mut self, cleanup_state: PersistCleanupState) -> Self {
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
