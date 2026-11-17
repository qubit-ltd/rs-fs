// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Condvar;
use std::sync::Mutex;
use std::task::Waker;

use super::super::FsEffectSnapshot;

/// Shared synchronization state behind mutation-effect observers.
pub(in crate::execution::effect_observer) struct FsEffectShared {
    /// Latest immutable pending or final evidence.
    pub(in crate::execution::effect_observer) snapshot: Mutex<FsEffectSnapshot>,
    /// Notifies blocking waiters after final evidence is published.
    pub(in crate::execution::effect_observer) changed: Condvar,
    /// Async task wakers registered while evidence is still pending.
    pub(in crate::execution::effect_observer) wakers: Mutex<Vec<Waker>>,
}
impl FsEffectShared {
    /// Creates a pending snapshot with empty synchronous and asynchronous
    /// waiters.
    pub(in crate::execution::effect_observer) fn new() -> Self {
        Self {
            snapshot: Mutex::new(FsEffectSnapshot::Pending),
            changed: Condvar::new(),
            wakers: Mutex::new(Vec::new()),
        }
    }
}
