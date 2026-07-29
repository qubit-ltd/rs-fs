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
}

impl CreateDirectoryOutcome {
    /// Creates an outcome. `already_existed` reports an accepted existing
    /// directory.
    #[must_use]
    pub const fn new(already_existed: bool) -> Self {
        Self { already_existed }
    }

    /// Returns whether an existing directory satisfied the request.
    #[must_use]
    pub const fn already_existed(self) -> bool {
        self.already_existed
    }
}
