// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use super::effect_slot_lease::EffectSlotLease;
use super::fs_effect_shared::FsEffectShared;

/// Snapshot state paired with the lease that keeps its capacity reserved.
pub(in crate::execution::effect_observer) struct EffectSlotReservationInner {
    /// Shared evidence observed by observer clones.
    pub(in crate::execution::effect_observer) state: Arc<FsEffectShared>,
    /// Lease retained until the reservation and prepared work are dropped.
    pub(in crate::execution::effect_observer) _slot: EffectSlotLease,
}

impl Drop for EffectSlotReservationInner {
    /// Marks an unpublished effect closed before its capacity lease is returned.
    fn drop(&mut self) {
        self.state.close();
    }
}
