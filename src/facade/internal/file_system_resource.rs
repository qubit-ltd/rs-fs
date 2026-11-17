// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Resource categories counted by facade byte budgets.

/// Distinguishes the byte streams tracked by filesystem facade budgets.
///
/// Separate categories keep source bytes read independent from destination
/// bytes accepted by a writer, so each operation enforces its own cumulative
/// limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSystemResource {
    /// Source bytes delivered to a read operation.
    ReadBytes,
    /// Destination bytes accepted by the provider's writer.
    WriteBytes,
}
