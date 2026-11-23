// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::FsEffectFact;

/// Immutable state of an independently observed mutation effect.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectSlotPool;
/// use qubit_fs::FsEffectSnapshot;
///
/// let reservation = FsEffectSlotPool::new(1).reserve().expect("slot available");
/// let observer = reservation.observer();
/// assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
/// ```
#[derive(Clone, Debug)]
#[must_use]
pub enum FsEffectSnapshot {
    /// Final evidence has not arrived.
    Pending,
    /// The last publisher was dropped before final evidence arrived, so the
    /// effect is unknown and no further evidence can be published.
    Closed,
    /// Immutable typed effect evidence published by the provider.
    Final(
        /// Typed fact captured at the first successful publication.
        FsEffectFact,
    ),
}
