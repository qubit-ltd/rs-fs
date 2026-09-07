// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Resource categories counted by facade byte budgets.

/// Resources counted by filesystem facade byte budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSystemResource {
    /// Bytes read from a source.
    ReadBytes,
    /// Bytes accepted by a destination writer.
    WriteBytes,
}
