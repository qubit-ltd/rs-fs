// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider operation phases observed by aggregate reads.

/// Provider operation being performed by an aggregate read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadIoPhase {
    /// Opening the reader.
    Open,
    /// Reading a chunk from the reader.
    Read,
}
