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

use super::FsEffectCapacityError;
use super::FsEffectReservation;
use super::internal::EffectSlotLease;
use super::internal::EffectSlotPoolInner;
use super::internal::EffectSlotReservationInner;
use super::internal::FsEffectShared;

/// Bounded pool that reserves one final-effect fact slot before mutation I/O.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectSlotPool;
///
/// let pool = FsEffectSlotPool::new(2);
/// let first = pool.reserve().expect("first slot available");
/// let second = pool.reserve().expect("second slot available");
/// let _reservations = (first, second);
/// ```
#[must_use = "retain the pool to reserve mutation effect slots"]
#[derive(Clone)]
pub struct FsEffectSlotPool {
    /// Shared capacity counter and configured limit.
    inner: Arc<EffectSlotPoolInner>,
}
impl FsEffectSlotPool {
    /// Creates a pool with a fixed maximum number of simultaneous fact slots.
    ///
    /// # Parameters
    ///
    /// - `capacity`: Maximum number of reservations that may be live at once.
    ///
    /// # Returns
    ///
    /// A pool that rejects new reservations while all `capacity` leases are
    /// retained.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(EffectSlotPoolInner {
                capacity,
                used: Arc::new(AtomicUsize::new(0)),
            }),
        }
    }
    /// Reserves a fact slot, or returns capacity exhaustion before I/O.
    ///
    /// # Returns
    ///
    /// A reservation owning one slot lease and a fresh pending snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`FsEffectCapacityError`] when all configured slot leases are
    /// currently held.
    pub fn reserve(&self) -> Result<FsEffectReservation, FsEffectCapacityError> {
        let used = &self.inner.used;
        let result = used.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < self.inner.capacity).then(|| n + 1)
        });
        if result.is_err() {
            return Err(FsEffectCapacityError);
        }
        Ok(FsEffectReservation {
            inner: Arc::new(EffectSlotReservationInner {
                state: Arc::new(FsEffectShared::new()),
                _slot: EffectSlotLease { used: Arc::clone(used) },
            }),
        })
    }
}
impl Default for FsEffectSlotPool {
    /// Creates a pool with the default capacity of 64 simultaneous slots.
    ///
    /// # Returns
    ///
    /// A pool configured to reserve at most 64 simultaneous effect slots.
    fn default() -> Self {
        Self::new(64)
    }
}
