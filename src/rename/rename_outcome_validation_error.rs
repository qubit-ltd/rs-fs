// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- covered through the public rename
// facade validation tests.
//! Provider rename outcome validation error details.

use super::RenameFailureState;

/// A provider rename outcome violation and the destination state it implies.
pub(crate) struct RenameOutcomeValidationError {
    /// Safe contract-violation message.
    pub(crate) message: &'static str,
    /// Strongest state known after the invalid provider outcome.
    pub(crate) state: RenameFailureState,
}
