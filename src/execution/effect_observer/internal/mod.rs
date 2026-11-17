// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private shared state and leases for effect observation.

mod effect_slot_lease;
mod effect_slot_pool_inner;
mod effect_slot_reservation_inner;
mod fs_effect_shared;

pub(super) use effect_slot_lease::EffectSlotLease;
pub(super) use effect_slot_pool_inner::EffectSlotPoolInner;
pub(super) use effect_slot_reservation_inner::EffectSlotReservationInner;
pub(super) use fs_effect_shared::FsEffectShared;
