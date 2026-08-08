// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recovery-state mapping shared by synchronous and asynchronous copy fallback.
// qubit-style: allow source-test-pair
// The facade-level mappings are covered by copy_fallback_tests.rs and
// async_copy_fallback_tests.rs.

use crate::CopyFailureState;
use crate::CopyStats;
use crate::WriteFailureState;
use crate::WriterState;

/// Maps an opened writer lifecycle state to a copy recovery state.
///
/// The writer SPI publishes accepted bytes only at commit. An open,
/// not-published, or aborted writer therefore has not published a destination
/// effect.
///
/// # Parameters
///
/// - `state`: Current lifecycle state of the fallback destination writer.
///
/// # Returns
///
/// The copy recovery state implied by the writer publication certainty.
#[inline(always)]
pub(crate) const fn from_writer_state(state: WriterState) -> CopyFailureState {
    match state {
        WriterState::Open
        | WriterState::NotPublished
        | WriterState::Aborted => CopyFailureState::Unchanged,
        WriterState::Committed | WriterState::Published => {
            CopyFailureState::Published
        }
        WriterState::Indeterminate => CopyFailureState::Indeterminate,
    }
}

/// Maps a provider-confirmed write-commit failure to a copy recovery state.
///
/// # Parameters
///
/// - `state`: Publication certainty reported by the provider commit attempt.
///
/// # Returns
///
/// The equivalent recovery state for the enclosing copy operation.
#[inline(always)]
pub(crate) const fn from_write_failure_state(
    state: WriteFailureState,
) -> CopyFailureState {
    match state {
        WriteFailureState::RetryableNotPublished
        | WriteFailureState::NotPublished => CopyFailureState::Unchanged,
        WriteFailureState::Published => CopyFailureState::Published,
        WriteFailureState::Indeterminate => CopyFailureState::Indeterminate,
    }
}

/// Builds failure statistics after a fallback destination writer was opened.
///
/// # Parameters
///
/// - `bytes`: Bytes accepted by the destination writer before the failure.
///
/// # Returns
///
/// Failure statistics retaining the observed byte count.
#[inline(always)]
pub(crate) const fn fallback_failure_stats(bytes: u64) -> CopyStats {
    CopyStats {
        files: 0,
        directories: 0,
        symlinks: 0,
        objects: 0,
        prefixes: 0,
        bytes,
        overwritten: 0,
        skipped: 0,
        failed: 1,
    }
}
