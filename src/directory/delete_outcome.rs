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

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::DeleteOutcome;

    #[test]
    fn outcome_accessors_are_executed_at_runtime() {
        let constructor: fn(bool) -> DeleteOutcome = black_box(DeleteOutcome::new);
        let with_entries: fn(DeleteOutcome, u64) -> DeleteOutcome = black_box(DeleteOutcome::with_deleted_entries);
        let already_missing: fn(DeleteOutcome) -> bool = black_box(DeleteOutcome::already_missing);
        let deleted_entries: fn(DeleteOutcome) -> Option<u64> = black_box(DeleteOutcome::deleted_entries);

        let outcome = with_entries(constructor(true), 2);
        assert!(already_missing(outcome));
        assert_eq!(Some(2), deleted_entries(outcome));
    }
}
