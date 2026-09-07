// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stage-specific writer policy and asynchronous recovery helpers.
mod open_writer_failure;
#[cfg(feature = "async")]
mod write_all_cancellation_guard;
#[cfg(feature = "async")]
mod write_all_recovery_snapshot;

pub(crate) use open_writer_failure::is_unchanged_open_failure;
pub(crate) use open_writer_failure::open_failure_state;
#[cfg(feature = "async")]
pub(crate) use write_all_cancellation_guard::WriteAllCancellationGuard;
#[cfg(feature = "async")]
pub(crate) use write_all_recovery_snapshot::WriteAllRecoverySnapshot;
