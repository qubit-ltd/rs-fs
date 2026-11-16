use crate::error::FsEffectState;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::task::Waker;
/// Immutable state of an independently observed mutation effect.
#[derive(Clone, Debug)]
#[must_use]
pub enum FsEffectSnapshot {
    /// Final evidence has not arrived.
    Pending,
    /// Immutable typed effect evidence published by the provider.
    Final(FsEffectFact),
}

/// Typed final evidence retained independently of the operation outcome.
#[derive(Clone, Debug)]
pub enum FsEffectFact {
    /// Strongest confirmed storage effect.
    State(FsEffectState),
    /// Shareable copy facts, with physical recovery ownership held elsewhere.
    CopyFailure {
        /// Original causal filesystem error.
        error: Arc<crate::FsError>,
        /// Confirmed publication state.
        state: crate::copy::CopyFailureState,
        /// Confirmed transfer counters.
        partial_stats: crate::copy::CopyStats,
        /// Recovery record identifier, when one owns a retained resource.
        recovery: Option<qubit_operation::RecoveryId>,
    },
    /// Shareable write facts, with physical recovery ownership held elsewhere.
    WriteFailure {
        /// Original causal filesystem error.
        error: Arc<crate::FsError>,
        /// Confirmed publication state.
        state: crate::write::WriteFailureState,
        /// Confirmed byte count.
        written_bytes: u64,
        /// Recovery record identifier, when one owns a retained resource.
        recovery: Option<qubit_operation::RecoveryId>,
    },
    /// Shareable rename facts.
    RenameFailure {
        /// Original causal filesystem error.
        error: Arc<crate::FsError>,
        /// Provider-confirmed namespace transition.
        state: crate::rename::RenameFailureState,
    },
    /// Provider-confirmed staged-tree publication facts.
    TreePublish {
        /// Actual publication outcome, including partial counts and guarantees.
        outcome: Arc<crate::directory::TreePublishOutcome>,
        /// Recovery record retaining a failed publication owner, when present.
        recovery: Option<qubit_operation::RecoveryId>,
    },
}
/// Cloneable read-only port for observing a managed mutation effect.
#[derive(Clone)]
pub struct FsEffectObserver {
    state: Arc<FsEffectShared>,
}
struct FsEffectShared {
    snapshot: Mutex<FsEffectSnapshot>,
    changed: Condvar,
    wakers: Mutex<Vec<Waker>>,
}
impl FsEffectShared {
    fn new() -> Self {
        Self {
            snapshot: Mutex::new(FsEffectSnapshot::Pending),
            changed: Condvar::new(),
            wakers: Mutex::new(Vec::new()),
        }
    }
}
impl FsEffectObserver {
    /// Returns the latest effect snapshot without consuming it.
    pub fn snapshot(&self) -> FsEffectSnapshot {
        self.state
            .snapshot
            .lock()
            .map(|v| v.clone())
            .unwrap_or(FsEffectSnapshot::Pending)
    }
    /// Publishes final effect evidence to observer clones.
    pub fn publish(&self, state: FsEffectState) {
        self.publish_fact(FsEffectFact::State(state));
    }
    /// Publishes typed final evidence once; later publications cannot replace it.
    pub fn publish_fact(&self, fact: FsEffectFact) {
        let mut published = false;
        if let Ok(mut v) = self.state.snapshot.lock() {
            if matches!(*v, FsEffectSnapshot::Pending) {
                *v = FsEffectSnapshot::Final(fact);
                published = true;
            }
        }
        if published {
            self.state.changed.notify_all();
            if let Ok(mut wakers) = self.state.wakers.lock() {
                for waker in wakers.drain(..) {
                    waker.wake();
                }
            }
        }
    }
    /// Waits synchronously until typed final evidence is available.
    pub fn wait_final(&self) -> Result<FsEffectFact, FsEffectObservationError> {
        let mut snapshot = self
            .state
            .snapshot
            .lock()
            .map_err(|_| FsEffectObservationError::Poisoned)?;
        loop {
            match &*snapshot {
                FsEffectSnapshot::Final(fact) => return Ok(fact.clone()),
                FsEffectSnapshot::Pending => {
                    snapshot = self
                        .state
                        .changed
                        .wait(snapshot)
                        .map_err(|_| FsEffectObservationError::Poisoned)?;
                }
            }
        }
    }
    /// Returns a future that waits without blocking for typed final evidence.
    pub fn wait_final_async(&self) -> FsEffectWait<'_> {
        FsEffectWait { observer: self }
    }
}

/// Failure to observe a final effect fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsEffectObservationError {
    /// The shared fact lock was poisoned by a panicking thread.
    Poisoned,
}
impl std::fmt::Display for FsEffectObservationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("filesystem effect fact lock is poisoned")
    }
}
impl std::error::Error for FsEffectObservationError {}

/// Future returned by [`FsEffectObserver::wait_final_async`].
pub struct FsEffectWait<'a> {
    observer: &'a FsEffectObserver,
}
impl std::future::Future for FsEffectWait<'_> {
    type Output = Result<FsEffectFact, FsEffectObservationError>;
    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let observer = self.observer;
        match observer.state.snapshot.lock() {
            Err(_) => std::task::Poll::Ready(Err(FsEffectObservationError::Poisoned)),
            Ok(snapshot) => match &*snapshot {
                FsEffectSnapshot::Final(fact) => std::task::Poll::Ready(Ok(fact.clone())),
                FsEffectSnapshot::Pending => {
                    drop(snapshot);
                    let mut wakers = match observer.state.wakers.lock() {
                        Ok(wakers) => wakers,
                        Err(_) => {
                            return std::task::Poll::Ready(Err(FsEffectObservationError::Poisoned));
                        }
                    };
                    if !wakers.iter().any(|waker| waker.will_wake(cx.waker())) {
                        wakers.push(cx.waker().clone());
                    }
                    drop(wakers);
                    match observer.state.snapshot.lock() {
                        Err(_) => std::task::Poll::Ready(Err(FsEffectObservationError::Poisoned)),
                        Ok(snapshot) => match &*snapshot {
                            FsEffectSnapshot::Final(fact) => {
                                std::task::Poll::Ready(Ok(fact.clone()))
                            }
                            FsEffectSnapshot::Pending => std::task::Poll::Pending,
                        },
                    }
                }
            },
        }
    }
}
struct EffectSlotLease {
    used: Arc<AtomicUsize>,
}
impl Drop for EffectSlotLease {
    fn drop(&mut self) {
        self.used.fetch_sub(1, Ordering::AcqRel);
    }
}
struct EffectSlotPoolInner {
    capacity: usize,
    used: Arc<AtomicUsize>,
}
struct EffectSlotReservationInner {
    state: Arc<FsEffectShared>,
    _slot: EffectSlotLease,
}
/// Reserved bounded fact slot retained by active prepared work.
#[derive(Clone)]
pub struct FsEffectReservation {
    inner: Arc<EffectSlotReservationInner>,
}
impl FsEffectReservation {
    /// Creates an observer port without transferring the active slot lease.
    pub fn observer(&self) -> FsEffectObserver {
        FsEffectObserver {
            state: Arc::clone(&self.inner.state),
        }
    }
    /// Publishes final effect evidence while retaining the slot lease.
    pub fn publish(&self, fact: FsEffectFact) {
        self.observer().publish_fact(fact);
    }
}
/// Bounded pool that reserves one final-effect fact slot before mutation I/O.
#[derive(Clone)]
pub struct FsEffectSlotPool {
    inner: Arc<EffectSlotPoolInner>,
}
/// Error returned when all bounded effect slots are reserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FsEffectCapacityError;
impl FsEffectSlotPool {
    /// Creates a pool with a fixed maximum number of simultaneous fact slots.
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(EffectSlotPoolInner {
                capacity,
                used: Arc::new(AtomicUsize::new(0)),
            }),
        }
    }
    /// Reserves a fact slot, or returns capacity exhaustion before I/O.
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
                _slot: EffectSlotLease {
                    used: Arc::clone(used),
                },
            }),
        })
    }
}
impl Default for FsEffectSlotPool {
    fn default() -> Self {
        Self::new(64)
    }
}
impl std::fmt::Display for FsEffectCapacityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("filesystem effect fact capacity exhausted")
    }
}
impl std::error::Error for FsEffectCapacityError {}

/// Mutation handle paired with its independent effect observer.
pub struct FsMutationOperation<T: Send + 'static> {
    /// Managed operation handle.
    pub operation: super::FsOperation<T>,
    /// Independent mutation effect port.
    pub effect: FsEffectObserver,
}
/// Async mutation handle paired with its independent effect observer.
pub struct AsyncFsMutationOperation<T: Send + 'static> {
    /// Managed async operation handle.
    pub operation: super::AsyncFsOperation<T>,
    /// Independent mutation effect port.
    pub effect: FsEffectObserver,
}
