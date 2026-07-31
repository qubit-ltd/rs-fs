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
mod internal;

pub use async_copy_failure::AsyncCopyFailure;
pub use async_copy_operation::AsyncCopyOperation;
pub use async_copy_operation_state::AsyncCopyOperationState;
pub use copy_failure::CopyFailure;
pub use copy_failure_state::CopyFailureState;

pub(crate) use internal::{
    fallback_options_supported,
    fallback_failure_stats,
    from_write_failure_state,
    from_writer_state,
    is_file_kind_supported,
    validate_stream_copy_length_limits,
};
