// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Deletion outcome.

/// Result returned after a deletion request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeleteOutcome {
    already_missing: bool,
}

impl DeleteOutcome {
    /// Creates an outcome. `already_missing` reports an accepted missing target.
    #[must_use]
    pub const fn new(already_missing: bool) -> Self {
        Self { already_missing }
    }

    /// Returns whether a missing target satisfied the request.
    #[must_use]
    pub const fn already_missing(self) -> bool {
        self.already_missing
    }
}
