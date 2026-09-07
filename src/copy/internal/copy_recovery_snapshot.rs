// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private recovery facts retained by an owning asynchronous copy operation.
use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
/// Publication certainty and progress retained independently of execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyRecoverySnapshot {
    /// Last confirmed destination publication state.
    pub(crate) state: CopyFailureState,
    /// Confirmed copy progress at the recorded state.
    pub(crate) stats: CopyStats,
}
impl CopyRecoverySnapshot {
    /// Creates the initial no-effect, zero-progress snapshot.
    #[inline]
    pub(crate) const fn unchanged() -> Self {
        Self {
            state: CopyFailureState::Unchanged,
            stats: CopyStats {
                files: 0,
                directories: 0,
                symlinks: 0,
                objects: 0,
                prefixes: 0,
                bytes: 0,
                overwritten: 0,
                skipped: 0,
                failed: 0,
            },
        }
    }
}
