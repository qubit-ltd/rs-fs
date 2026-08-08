// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation details for copy orchestration.

#[cfg(feature = "async")]
mod copy_cancellation_guard;
mod fallback_failure_state;
mod stream_copy_policy;

#[cfg(feature = "async")]
pub(super) use copy_cancellation_guard::CopyCancellationGuard;
pub(crate) use fallback_failure_state::fallback_failure_stats;
pub(crate) use fallback_failure_state::from_write_failure_state;
pub(crate) use fallback_failure_state::from_writer_state;
pub(crate) use stream_copy_policy::fallback_options_supported;
pub(crate) use stream_copy_policy::is_file_kind_supported;
pub(crate) use stream_copy_policy::validate_stream_copy_length_limits;
