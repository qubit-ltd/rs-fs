// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Atomicity actually achieved by a completed operation.

/// Atomicity guarantee actually achieved by a successful operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AchievedAtomicity {
    /// The externally visible state transition was atomic.
    Atomic,
    /// The operation completed through a non-atomic sequence.
    NonAtomic,
}
