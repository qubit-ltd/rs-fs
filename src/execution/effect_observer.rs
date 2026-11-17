// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Typed mutation-effect observation and bounded reservations.

mod async_fs_mutation_operation;
mod fs_effect_capacity_error;
mod fs_effect_fact;
mod fs_effect_observation_error;
mod fs_effect_observer;
mod fs_effect_reservation;
mod fs_effect_slot_pool;
mod fs_effect_snapshot;
mod fs_effect_wait;
mod fs_mutation_operation;
mod internal;

pub use async_fs_mutation_operation::AsyncFsMutationOperation;
pub use fs_effect_capacity_error::FsEffectCapacityError;
pub use fs_effect_fact::FsEffectFact;
pub use fs_effect_observation_error::FsEffectObservationError;
pub use fs_effect_observer::FsEffectObserver;
pub use fs_effect_reservation::FsEffectReservation;
pub use fs_effect_slot_pool::FsEffectSlotPool;
pub use fs_effect_snapshot::FsEffectSnapshot;
pub use fs_effect_wait::FsEffectWait;
pub use fs_mutation_operation::FsMutationOperation;
