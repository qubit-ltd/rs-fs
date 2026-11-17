// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Write operation outcome.

use crate::metadata::AchievedAtomicity;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::PublicationMethod;
use crate::metadata::ResourceVersion;
use crate::metadata::UserMetadata;

/// Outcome returned when a writer is committed.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::AchievedAtomicity;
/// use qubit_fs::metadata::PublicationMethod;
/// use qubit_fs::metadata::WriteOutcome;
///
/// let outcome = WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct);
/// assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct WriteOutcome {
    /// Number of bytes written when known.
    bytes_written: Option<u64>,
    /// Provider version, generation, or ETag when known.
    version: Option<ResourceVersion>,
    /// Atomicity actually achieved by publication.
    atomicity: AchievedAtomicity,
    /// Concrete publication method that completed the write.
    method: PublicationMethod,
    /// Whether data and namespace publication were durably synchronized.
    durable: bool,
    /// Provider diagnostics whose keys passed sensitivity checks.
    diagnostics: NonSensitiveMetadata,
}

impl WriteOutcome {
    /// Creates a write outcome with explicit publication semantics.
    ///
    /// # Parameters
    /// - `atomicity`: Atomicity actually achieved.
    /// - `method`: Method used to publish the resource.
    ///
    /// # Returns
    /// A write outcome with no byte count, version, or diagnostics.
    #[inline]
    #[must_use]
    pub fn new(atomicity: AchievedAtomicity, method: PublicationMethod) -> Self {
        Self {
            bytes_written: None,
            version: None,
            atomicity,
            method,
            durable: false,
            diagnostics: NonSensitiveMetadata::new(),
        }
    }

    /// Returns the number of bytes accepted by the write session, when known.
    ///
    /// # Returns
    /// `Some(bytes)` when the provider reported a count, or `None` otherwise.
    #[inline]
    pub const fn bytes_written(&self) -> Option<u64> {
        self.bytes_written
    }

    /// Returns the provider version, generation, or ETag when known.
    ///
    /// # Returns
    /// `Some(version)` when reported, or `None` when the provider supplied no
    /// token.
    #[inline]
    pub const fn version(&self) -> Option<&ResourceVersion> {
        self.version.as_ref()
    }

    /// Returns the atomicity actually achieved by publication.
    ///
    /// # Returns
    /// The atomicity classification recorded for this write.
    #[inline]
    #[must_use]
    pub const fn atomicity(&self) -> AchievedAtomicity {
        self.atomicity
    }

    /// Returns the concrete method that published the resource.
    ///
    /// # Returns
    /// The publication method recorded for this write.
    #[inline]
    #[must_use]
    pub const fn method(&self) -> PublicationMethod {
        self.method
    }

    /// Returns whether the provider confirmed durable publication.
    ///
    /// # Returns
    /// `true` when the provider confirmed durable synchronization.
    #[inline]
    #[must_use]
    pub const fn durable(&self) -> bool {
        self.durable
    }

    /// Returns provider diagnostics whose keys passed sensitivity checks.
    ///
    /// Values are retained unchanged. Providers are responsible for sanitizing
    /// diagnostic values before storing them; explicitly reading a value does
    /// not make it safe to log.
    ///
    /// # Returns
    /// A shared view of the diagnostics wrapper; values remain unchanged and
    /// must still be sanitized before logging.
    #[inline]
    #[must_use]
    pub const fn diagnostics(&self) -> &NonSensitiveMetadata {
        &self.diagnostics
    }

    /// Records the byte count confirmed by the provider.
    ///
    /// # Parameters
    /// - `bytes_written`: Number of bytes accepted by the committed write.
    ///
    /// # Returns
    /// The updated outcome with the provider-confirmed byte count.
    #[inline]
    #[must_use]
    pub const fn with_bytes_written(mut self, bytes_written: u64) -> Self {
        self.bytes_written = Some(bytes_written);
        self
    }

    /// Records the provider version, generation, or ETag.
    ///
    /// # Parameters
    /// - `version`: Provider token identifying the resulting resource version.
    ///
    /// # Returns
    /// The updated outcome retaining the provider version token.
    #[inline]
    #[must_use]
    pub fn with_version(mut self, version: ResourceVersion) -> Self {
        self.version = Some(version);
        self
    }

    /// Records whether data and namespace publication were durably
    /// synchronized.
    ///
    /// # Parameters
    /// - `durable`: Whether the provider confirmed durable publication.
    ///
    /// # Returns
    /// The updated outcome with the provider's durability confirmation.
    #[inline]
    #[must_use]
    pub const fn with_durable(mut self, durable: bool) -> Self {
        self.durable = durable;
        self
    }

    /// Replaces provider diagnostics after their values have been sanitized.
    ///
    /// Keys are checked when added, while values are retained unchanged.
    ///
    /// # Parameters
    /// - `diagnostics`: Provider diagnostics with values already sanitized by
    ///   the provider.
    ///
    /// # Returns
    /// The updated outcome retaining the sanitized diagnostics wrapper.
    #[inline]
    #[must_use]
    pub fn with_diagnostics(mut self, diagnostics: UserMetadata) -> Self {
        self.diagnostics = NonSensitiveMetadata::from(diagnostics);
        self
    }
}
