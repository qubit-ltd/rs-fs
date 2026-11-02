// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Rename operation outcome.

use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::PublicationMethod;
use crate::metadata::UserMetadata;
use crate::path::Path;

/// Outcome of a rename, move, or provider-equivalent publication.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{AchievedAtomicity, PublicationMethod};
/// use qubit_fs::path::Path;
/// use qubit_fs::rename::RenameOutcome;
///
/// let outcome = RenameOutcome::new(
///     Path::parse("/src")?,
///     Path::parse("/dest")?,
///     AchievedAtomicity::Atomic,
///     PublicationMethod::Direct,
/// );
/// assert_eq!("/dest", outcome.target().as_str());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct RenameOutcome {
    /// Atomicity achieved while publishing the destination.
    atomicity: AchievedAtomicity,
    /// Actual publication mechanism used.
    method: PublicationMethod,
    /// Whether the provider synchronized durable destination publication.
    durable: bool,
    /// Source identity confirmed by the provider.
    source: Path,
    /// Destination identity confirmed by the provider.
    target: Path,
    /// Provider diagnostics whose keys passed sensitivity checks.
    diagnostics: NonSensitiveMetadata,
}

impl RenameOutcome {
    /// Creates a rename outcome with explicit successful semantics.
    ///
    /// # Parameters
    /// - `source`: Source identity supplied by the provider.
    /// - `target`: Target identity supplied by the provider.
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the destination.
    ///
    /// # Returns
    /// A rename outcome without diagnostics.
    #[inline]
    #[must_use]
    pub fn new(source: Path, target: Path, atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            atomicity,
            method,
            durable: false,
            source,
            target,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Replaces the provider-reported durability completion fact.
    ///
    /// # Parameters
    ///
    /// - `durable`: Whether destination publication was synchronized.
    ///
    /// # Returns
    ///
    /// This outcome with its durability fact replaced.
    #[inline]
    #[must_use]
    pub fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Replaces provider diagnostics after their values have been sanitized.
    ///
    /// Keys are checked when added, while values are retained unchanged.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }
    /// Returns the source identity.
    #[inline]
    #[must_use]
    pub fn source(&self) -> &Path {
        &self.source
    }
    /// Returns the target identity.
    #[inline]
    #[must_use]
    pub fn target(&self) -> &Path {
        &self.target
    }
    /// Returns actual publication atomicity.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }
    /// Returns the provider's publication method.
    #[inline]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns whether the provider synchronized durable destination
    /// publication.
    #[inline]
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }
    /// Returns provider diagnostics whose keys passed sensitivity checks.
    ///
    /// Values are retained unchanged. Providers must sanitize diagnostic
    /// values before storing them; explicitly reading a value does not make it
    /// safe to log.
    #[inline]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }
}
