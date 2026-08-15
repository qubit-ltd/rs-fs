// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed copy failure state shared across the facade and provider boundary.

#[cfg(feature = "async")]
mod async_copy_failure;
#[cfg(feature = "async")]
mod async_copy_operation;
#[cfg(feature = "async")]
mod async_copy_operation_state;
mod copy_conflict_policy;
mod copy_failure;
mod copy_failure_state;
mod copy_method;
mod copy_mode;
mod copy_operation;
mod copy_options;
mod copy_outcome;
mod copy_stats;
mod internal;
mod metadata_preserve_policy;
mod server_side_preference;

#[cfg(feature = "async")]
pub use async_copy_failure::AsyncCopyFailure;
#[cfg(feature = "async")]
pub use async_copy_operation::AsyncCopyOperation;
#[cfg(feature = "async")]
pub use async_copy_operation_state::AsyncCopyOperationState;
pub use copy_conflict_policy::CopyConflictPolicy;
pub use copy_failure::CopyFailure;
pub use copy_failure_state::CopyFailureState;
pub use copy_method::CopyMethod;
pub use copy_mode::CopyMode;
pub(crate) use copy_operation::CopyOperation;
pub use copy_options::CopyOptions;
pub use copy_outcome::CopyOutcome;
pub use copy_stats::CopyStats;
pub(crate) use internal::fallback_failure_stats;
pub(crate) use internal::fallback_options_supported;
pub(crate) use internal::from_write_failure_state;
pub(crate) use internal::from_writer_state;
pub(crate) use internal::is_file_kind_supported;
pub(crate) use internal::validate_stream_copy_length_limits;
pub use metadata_preserve_policy::MetadataPreservePolicy;
pub use server_side_preference::ServerSidePreference;
