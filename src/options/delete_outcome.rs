// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Deletion outcome.

/// Result returned after a deletion request.
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
    #[inline]
    #[must_use]
    pub const fn new(already_missing: bool) -> Self {
        Self {
            already_missing,
            deleted_entries: None,
        }
    }

    /// Returns whether a missing target satisfied the request.
    #[inline(always)]
    #[must_use]
    pub const fn already_missing(self) -> bool {
        self.already_missing
    }

    /// Attaches the number of deleted entries, when known.
    #[inline(always)]
    #[must_use]
    pub const fn with_deleted_entries(mut self, count: u64) -> Self {
        self.deleted_entries = Some(count);
        self
    }

    /// Returns the number of deleted entries, when reported.
    #[inline(always)]
    #[must_use]
    pub const fn deleted_entries(self) -> Option<u64> {
        self.deleted_entries
    }
}
