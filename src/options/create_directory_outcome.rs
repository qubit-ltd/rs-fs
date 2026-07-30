// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Directory creation outcome.

/// Result returned after a directory creation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateDirectoryOutcome {
    already_existed: bool,
    created_ancestors: Option<u64>,
}

impl CreateDirectoryOutcome {
    /// Creates an outcome. `already_existed` reports an accepted existing
    /// directory.
    #[must_use]
    pub const fn new(already_existed: bool) -> Self {
        Self {
            already_existed,
            created_ancestors: None,
        }
    }

    /// Returns whether an existing directory satisfied the request.
    #[must_use]
    pub const fn already_existed(self) -> bool {
        self.already_existed
    }

    /// Attaches the number of ancestor directories created, when known.
    #[must_use]
    pub const fn with_created_ancestors(mut self, count: u64) -> Self {
        self.created_ancestors = Some(count);
        self
    }

    /// Returns the number of created ancestor directories, when reported.
    #[must_use]
    pub const fn created_ancestors(self) -> Option<u64> {
        self.created_ancestors
    }
}
