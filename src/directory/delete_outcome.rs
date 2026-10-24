// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deletion outcome.

/// Result returned after a deletion request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::directory::DeleteOutcome;
///
/// let outcome = DeleteOutcome::new(true);
/// assert!(outcome.already_missing());
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeleteOutcome {
    /// Whether an already-missing target satisfied the request.
    already_missing: bool,
    /// Number of deleted entries when the provider reports it.
    deleted_entries: Option<u64>,
}

impl DeleteOutcome {
    /// Creates an outcome. `already_missing` reports an accepted missing
    /// target.
    ///
    /// # Parameters
    ///
    /// * `already_missing` indicates whether an already-missing target
    ///   satisfied the request.
    ///
    /// # Returns
    ///
    /// A new outcome without a provider-reported deletion count.
    #[inline]
    #[must_use]
    pub const fn new(already_missing: bool) -> Self {
        Self {
            already_missing,
            deleted_entries: None,
        }
    }

    /// Returns whether a missing target satisfied the request.
    ///
    /// # Returns
    ///
    /// `true` when an already-missing target satisfied the request.
    #[inline]
    #[must_use]
    pub const fn already_missing(self) -> bool {
        self.already_missing
    }

    /// Returns the number of deleted entries, when reported.
    ///
    /// # Returns
    ///
    /// `Some(count)` when the provider reported the number of deleted entries,
    /// or `None` when it did not.
    #[inline]
    #[must_use]
    pub const fn deleted_entries(self) -> Option<u64> {
        self.deleted_entries
    }

    /// Attaches the number of deleted entries, when known.
    ///
    /// # Parameters
    ///
    /// * `count` is the number of entries deleted.
    ///
    /// # Returns
    ///
    /// The updated outcome with the reported deletion count.
    #[inline]
    #[must_use]
    pub const fn with_deleted_entries(mut self, count: u64) -> Self {
        self.deleted_entries = Some(count);
        self
    }
}
