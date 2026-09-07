// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared cooperative deadline tracking for copy operations.

use std::time::Duration;
use std::time::Instant;

/// Tracks one copy operation's cumulative elapsed-time budget.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CopyDeadline {
    /// Monotonic instant at which the operation was constructed.
    started_at: Instant,
    /// Optional cumulative elapsed-time budget.
    maximum: Option<Duration>,
}

impl CopyDeadline {
    /// Starts tracking a copy operation.
    #[inline]
    pub(crate) fn new(maximum: Option<Duration>) -> Self {
        Self {
            started_at: Instant::now(),
            maximum,
        }
    }

    /// Returns whether the cumulative budget has expired.
    #[inline]
    pub(crate) fn expired(self) -> bool {
        self.maximum
            .is_some_and(|maximum| self.started_at.elapsed() >= maximum)
    }
}
