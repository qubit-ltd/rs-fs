// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Directory stream lifecycle states.

/// Lifecycle state of a directory enumeration handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum DirectoryStreamState {
    /// The stream may still produce entries.
    Open,
    /// The provider reported the end of enumeration.
    Exhausted,
    /// Enumeration stopped because validation or provider I/O failed.
    Failed,
}
