// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

/// Pool capacity and shared counter retained by pool clones and leases.
pub(in crate::execution::effect_observer) struct EffectSlotPoolInner {
    /// Maximum number of simultaneous leases.
    pub(in crate::execution::effect_observer) capacity: usize,
    /// Number of leases currently held across all pool clones.
    pub(in crate::execution::effect_observer) used: Arc<AtomicUsize>,
}
