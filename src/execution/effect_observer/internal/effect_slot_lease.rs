// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

/// RAII lease that returns one reserved effect slot when its last owner drops.
pub(in crate::execution::effect_observer) struct EffectSlotLease {
    /// Shared count of currently reserved pool slots.
    pub(in crate::execution::effect_observer) used: Arc<AtomicUsize>,
}
impl Drop for EffectSlotLease {
    /// Releases this lease from the shared reservation count.
    fn drop(&mut self) {
        self.used.fetch_sub(1, Ordering::AcqRel);
    }
}
