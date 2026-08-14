// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bounded-read policy shared by facade implementations.

/// Fixed I/O chunk used by bounded prefix reads.
pub(crate) const PREFIX_BUFFER_SIZE: usize = 8192;

/// Returns the next bounded read length for an accumulated prefix.
#[inline(always)]
pub(crate) fn next_read_len(accumulated: usize, maximum: usize) -> usize {
    maximum.saturating_sub(accumulated).min(PREFIX_BUFFER_SIZE)
}
