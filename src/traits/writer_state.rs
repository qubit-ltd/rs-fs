// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! File writer lifecycle states.

/// Observable lifecycle state of a synchronous or asynchronous file writer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriterState {
    /// The session accepts bytes and may be committed or aborted.
    Open,
    /// Publication completed successfully.
    Committed,
    /// The session was explicitly cancelled and cleaned up.
    Aborted,
    /// Publication or lifecycle cleanup may have occurred, but the provider
    /// cannot confirm the final state.
    Indeterminate,
}
