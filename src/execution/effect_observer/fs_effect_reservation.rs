// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use qubit_operation::RecoveryId;

use super::FsEffectFact;
use super::FsEffectObserver;
use super::internal::EffectSlotReservationInner;
use crate::directory::TreePublishOutcome;

/// Reserved bounded fact slot retained by active prepared work.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectFact;
/// use qubit_fs::FsEffectSlotPool;
/// use qubit_fs::FsEffectSnapshot;
/// use qubit_fs::FsEffectState;
///
/// let reservation = FsEffectSlotPool::new(1).reserve().expect("slot available");
/// let observer = reservation.observer();
/// reservation.publish(FsEffectFact::State(FsEffectState::Applied));
/// assert!(matches!(observer.snapshot(), FsEffectSnapshot::Final(_)));
/// ```
#[derive(Clone)]
pub struct FsEffectReservation {
    /// Shared snapshot and capacity lease retained by this reservation.
    pub(super) inner: Arc<EffectSlotReservationInner>,
}
impl FsEffectReservation {
    /// Creates an observer port without transferring the active slot lease.
    ///
    /// # Returns
    /// A read-only observer sharing this reservation's pending/final snapshot.
    #[must_use = "use the observer to inspect or wait for the effect"]
    #[inline]
    pub fn observer(&self) -> FsEffectObserver {
        FsEffectObserver {
            state: Arc::clone(&self.inner.state),
        }
    }
    /// Publishes final effect evidence while retaining the slot lease.
    ///
    /// # Parameters
    /// - `fact`: Typed provider evidence to publish to observers.
    ///
    /// The reservation keeps its capacity lease until the reservation and its
    /// active prepared work are dropped.
    #[inline]
    pub fn publish(&self, fact: FsEffectFact) {
        self.inner.state.publish(fact);
    }
}

impl FsEffectReservation {
    /// Publishes typed write failure facts without taking the write outcome or
    /// retaining the writer that owns recovery responsibility.
    ///
    /// # Parameters
    /// - `error`: Confirmed write error.
    /// - `state`: Known write publication state.
    /// - `written_bytes`: Number of bytes confirmed written.
    /// - `recovery`: Optional recovery identifier.
    #[inline]
    pub fn publish_write_failure(
        &self,
        error: Arc<crate::FsError>,
        state: crate::write::WriteFailureState,
        written_bytes: u64,
        recovery: Option<RecoveryId>,
    ) {
        self.publish(crate::execution::FsEffectFact::WriteFailure {
            error,
            state,
            written_bytes,
            recovery,
        });
    }

    /// Publishes typed copy failure facts without taking the copy outcome or
    /// retaining a writer that owns recovery responsibility.
    ///
    /// # Parameters
    /// - `error`: Confirmed copy error.
    /// - `state`: Known copy publication state.
    /// - `partial_stats`: Copy progress confirmed before failure.
    /// - `recovery`: Optional recovery identifier.
    #[inline]
    pub fn publish_copy_failure(
        &self,
        error: Arc<crate::FsError>,
        state: crate::copy::CopyFailureState,
        partial_stats: crate::copy::CopyStats,
        recovery: Option<RecoveryId>,
    ) {
        self.publish(crate::execution::FsEffectFact::CopyFailure {
            error,
            state,
            partial_stats,
            recovery,
        });
    }

    /// Publishes typed rename failure facts independently of its outcome.
    ///
    /// # Parameters
    /// - `error`: Confirmed rename error.
    /// - `state`: Known rename publication state.
    #[inline]
    pub fn publish_rename_failure(&self, error: Arc<crate::FsError>, state: crate::rename::RenameFailureState) {
        self.publish(crate::execution::FsEffectFact::RenameFailure { error, state });
    }

    /// Publishes provider-confirmed tree publication facts independently of
    /// the operation outcome.
    ///
    /// # Parameters
    /// - `outcome`: Provider-confirmed tree publication details.
    /// - `recovery`: Optional recovery identifier for remaining cleanup work.
    #[inline]
    pub fn publish_tree_outcome(&self, outcome: TreePublishOutcome, recovery: Option<RecoveryId>) {
        self.publish(crate::execution::FsEffectFact::TreePublish {
            outcome: Arc::new(outcome),
            recovery,
        });
    }
}
