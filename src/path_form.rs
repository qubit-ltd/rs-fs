// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Permitted path forms for filesystem property snapshots.

/// Permitted absolute or relative form for paths accepted by a filesystem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathForm {
    /// Only absolute paths are accepted.
    Absolute,
    /// Only relative paths are accepted.
    Relative,
    /// Both absolute and relative paths are accepted.
    Either,
}
