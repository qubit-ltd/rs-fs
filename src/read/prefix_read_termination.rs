// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Completion state for bounded prefix reads.

/// Explains why a bounded prefix read stopped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PrefixReadTermination {
    /// The requested prefix limit was reached without probing beyond it.
    LimitReached,
    /// The opened stream returned EOF before the requested prefix limit.
    StreamEnded,
}
