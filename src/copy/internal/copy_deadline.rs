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
#[must_use]
#[derive(Clone, Copy, Debug)]
pub(crate) struct CopyDeadline {
    /// Monotonic instant at which the operation was constructed.
    started_at: Instant,
    /// Optional cumulative elapsed-time budget.
    maximum: Option<Duration>,
    /// Monotonic clock used for both the start instant and later checks.
    now: fn() -> Instant,
}

impl CopyDeadline {
    /// Starts tracking a copy operation.
    #[inline]
    #[must_use = "use the deadline to enforce the copy budget"]
    pub(crate) fn new(maximum: Option<Duration>, now: fn() -> Instant) -> Self {
        Self {
            started_at: now(),
            maximum,
            now,
        }
    }

    /// Returns whether the cumulative budget has expired.
    #[inline]
    #[must_use]
    pub(crate) fn expired(self) -> bool {
        self.maximum
            .is_some_and(|maximum| (self.now)().duration_since(self.started_at) >= maximum)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::time::Duration;
    use std::time::Instant;

    use super::CopyDeadline;

    thread_local! {
        static TEST_NOW: Cell<Option<Instant>> = const { Cell::new(None) };
    }

    fn test_now() -> Instant {
        TEST_NOW.with(|now| now.get().expect("test clock should be initialized"))
    }

    fn set_test_now(value: Instant) {
        TEST_NOW.with(|now| {
            if let Some(previous) = now.get() {
                assert!(value >= previous, "controlled test time must not move backwards");
            }
            now.set(Some(value));
        });
    }

    #[test]
    fn test_deadline_clock_boundaries_and_unbounded_budget() {
        let start = Instant::now();
        set_test_now(start);
        let unbounded = CopyDeadline::new(None, test_now);
        let deadline = CopyDeadline::new(Some(Duration::from_secs(2)), test_now);

        set_test_now(start + Duration::from_secs(2) - Duration::from_nanos(1));
        assert!(!deadline.expired());
        set_test_now(start + Duration::from_secs(2));
        assert!(deadline.expired());
        assert!(!unbounded.expired());
    }

    #[test]
    fn test_deadline_clock_never_moves_backwards_in_test() {
        let start = Instant::now();
        set_test_now(start);
        let deadline = CopyDeadline::new(Some(Duration::from_secs(1)), test_now);
        set_test_now(start + Duration::from_millis(1));
        assert!(!deadline.expired());
        let backwards = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            set_test_now(start);
        }));
        assert!(backwards.is_err(), "controlled test time must reject a backwards step");
    }
}
