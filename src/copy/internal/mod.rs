// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation details for copy orchestration.

mod copy_cancellation_guard;
mod fallback_failure_state;

pub(super) use copy_cancellation_guard::CopyCancellationGuard;
pub(crate) use fallback_failure_state::{
    fallback_failure_stats,
    from_write_failure_state,
    from_writer_state,
};
