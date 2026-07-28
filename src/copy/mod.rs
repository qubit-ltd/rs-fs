// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed copy failure state shared across the facade and provider boundary.

mod async_copy_failure;
mod async_copy_operation;
mod async_copy_operation_state;
mod copy_failure;
mod copy_failure_state;

pub use async_copy_failure::AsyncCopyFailure;
pub use async_copy_operation::AsyncCopyOperation;
pub use async_copy_operation_state::AsyncCopyOperationState;
pub use copy_failure::CopyFailure;
pub use copy_failure_state::CopyFailureState;
