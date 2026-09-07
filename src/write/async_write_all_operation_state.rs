// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! State for an owning asynchronous whole-file write.
use crate::write::WriteFailureState;
/// Lifecycle state of an owning asynchronous whole-file write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum AsyncWriteAllOperationState {
    /// The operation has not been polled.
    Ready,
    /// The provider call is in progress and has not completed.
    Running,
    /// The target was published successfully.
    Completed,
    /// The operation stopped with the paired publication state.
    Failed(WriteFailureState),
}
