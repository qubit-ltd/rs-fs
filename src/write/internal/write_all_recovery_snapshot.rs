// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recovery snapshot for async whole-file writes.

use crate::write::WriteFailureState;

/// Facts frozen when an owning write completes or is cancelled.
#[must_use = "the recovery snapshot must be retained for operation recovery"]
#[derive(Clone, Copy)]
pub(crate) struct WriteAllRecoverySnapshot {
    /// Last known publication certainty.
    pub(crate) state: WriteFailureState,
    /// Bytes acknowledged by completed successful writes.
    pub(crate) written_bytes: u64,
}

impl WriteAllRecoverySnapshot {
    /// Creates a snapshot before any provider work has started.
    ///
    /// # Returns
    /// A snapshot initialized to `NotPublished` with zero acknowledged bytes.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            state: WriteFailureState::NotPublished,
            written_bytes: 0,
        }
    }
}
