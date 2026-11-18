// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! One-shot asynchronous publication of a staged resource identity.

use std::future::Future;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;

use qubit_operation::ResourceId;

use super::staged_cleanup_identity_state::StagedCleanupIdentityState;

/// Gates cleanup until its runtime-assigned resource identity is available.
#[derive(Default)]
pub(in crate::spi::managed_tree) struct StagedCleanupIdentity {
    /// Shared ID and waiting task state; guarded only during poll or publish.
    state: Mutex<StagedCleanupIdentityState>,
}

impl StagedCleanupIdentity {
    /// Creates an identity gate that has not yet been published.
    pub(in crate::spi::managed_tree) fn new() -> Self {
        Self::default()
    }

    /// Publishes the resource identity and wakes a waiting cleanup future.
    ///
    /// Re-publishing the same identity is harmless. A different identity
    /// violates the one-owner registration contract and panics.
    ///
    /// # Parameters
    /// - `id`: Resource identity assigned by the operation scope.
    pub(in crate::spi::managed_tree) fn set(&self, id: ResourceId) {
        let waker = {
            let mut state = lock(&self.state);
            match state.id {
                Some(existing) => assert_eq!(existing, id, "staged cleanup identity cannot be replaced"),
                None => state.id = Some(id),
            }
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    /// Waits asynchronously until the resource identity is published.
    pub(in crate::spi::managed_tree) fn wait(&self) -> impl Future<Output = ResourceId> + '_ {
        std::future::poll_fn(move |context| {
            // Clone before taking the mutex; dropping a replaced Waker may run
            // arbitrary executor code, so it must happen after unlocking.
            let mut candidate = Some(context.waker().clone());
            let (id, replaced) = {
                let mut state = lock(&self.state);
                if let Some(id) = state.id {
                    (Some(id), None)
                } else {
                    let next = candidate.take().expect("candidate waker is present");
                    if state.waker.as_ref().is_some_and(|current| current.will_wake(&next)) {
                        (None, Some(next))
                    } else {
                        (None, state.waker.replace(next))
                    }
                }
            };
            drop(candidate);
            drop(replaced);
            id.map_or(std::task::Poll::Pending, std::task::Poll::Ready)
        })
    }
}

/// Acquires a gate mutex while recovering its inner state after poisoning.
///
/// # Type Parameters
/// - `T`: State protected by the mutex.
///
/// # Parameters
/// - `mutex`: Mutex guarding the identity gate state.
///
/// # Returns
/// The held guard, including when an earlier operation poisoned the mutex.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Wake;
    use std::task::Waker;

    use qubit_operation::ManagerOptions;
    use qubit_operation::OperationManager;
    use qubit_operation::OperationOutcome;
    use qubit_operation::ResourceScopeOptions;
    use qubit_operation::ShutdownMode;

    use super::StagedCleanupIdentity;

    #[derive(Default)]
    struct Counter(std::sync::atomic::AtomicUsize);

    impl Wake for Counter {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    struct ReentrantWake {
        gate: Arc<StagedCleanupIdentity>,
        observed_ready: std::sync::atomic::AtomicBool,
    }

    impl Wake for ReentrantWake {
        fn wake(self: Arc<Self>) {
            Wake::wake_by_ref(&self);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            let mut future = Box::pin(self.gate.wait());
            let mut context = Context::from_waker(Waker::noop());
            let ready = matches!(future.as_mut().poll(&mut context), Poll::Ready(_));
            self.observed_ready.store(ready, std::sync::atomic::Ordering::SeqCst);
        }
    }

    struct ReentrantDrop {
        gate: Arc<StagedCleanupIdentity>,
        observed_drop: Arc<std::sync::atomic::AtomicBool>,
        observed_wake: Arc<std::sync::atomic::AtomicBool>,
    }

    impl Wake for ReentrantDrop {
        fn wake(self: Arc<Self>) {
            self.observed_wake.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl Drop for ReentrantDrop {
        fn drop(&mut self) {
            let mut future = Box::pin(self.gate.wait());
            let mut context = Context::from_waker(Waker::noop());
            let observed = matches!(future.as_mut().poll(&mut context), Poll::Pending | Poll::Ready(_));
            self.observed_drop.store(observed, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn resource_id() -> qubit_operation::ResourceId {
        let manager = OperationManager::new(ManagerOptions::default()).unwrap();
        let mut operation = manager
            .try_submit(Default::default(), |operation_scope| {
                let scope = operation_scope
                    .resource_scope(ResourceScopeOptions { capacity: 1 })
                    .unwrap();
                let resource = match scope.reserve_resource().unwrap().commit(()) {
                    Ok(resource) => resource,
                    Err(_) => panic!("resource identity fixture commits"),
                };
                let id = resource.id();
                scope.end();
                Ok::<_, ()>(id)
            })
            .unwrap();
        let id = match operation.wait_outcome().unwrap() {
            OperationOutcome::Succeeded(id) => id,
            other => panic!("unexpected outcome: {other:?}"),
        };
        operation.wait_settlement().unwrap();
        let _shutdown_report = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
        id
    }

    #[test]
    fn test_wait_is_pending_until_set_then_wakes_with_the_published_id() {
        let gate = StagedCleanupIdentity::new();
        let id = resource_id();
        let counter = Arc::new(Counter::default());
        let waker = Waker::from(Arc::clone(&counter));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(gate.wait());
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
        gate.set(id);
        assert_eq!(counter.0.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Ready(actual) if actual == id));
        gate.set(id);
    }

    #[test]
    fn test_a_later_waiter_waker_replaces_the_previous_one() {
        let gate = StagedCleanupIdentity::new();
        let id = resource_id();
        let first = Arc::new(Counter::default());
        let second = Arc::new(Counter::default());
        let mut future = Box::pin(gate.wait());
        let first_waker = Waker::from(Arc::clone(&first));
        let second_waker = Waker::from(Arc::clone(&second));
        assert!(matches!(
            future.as_mut().poll(&mut Context::from_waker(&first_waker)),
            Poll::Pending
        ));
        assert!(matches!(
            future.as_mut().poll(&mut Context::from_waker(&second_waker)),
            Poll::Pending
        ));
        gate.set(id);
        assert_eq!(first.0.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(second.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_wake_callbacks_can_reenter_after_identity_publication() {
        let gate = Arc::new(StagedCleanupIdentity::new());
        let callback = Arc::new(ReentrantWake {
            gate: Arc::clone(&gate),
            observed_ready: std::sync::atomic::AtomicBool::new(false),
        });
        let waker = Waker::from(Arc::clone(&callback));
        let id = resource_id();
        let mut future = Box::pin(gate.wait());
        assert!(matches!(
            future.as_mut().poll(&mut Context::from_waker(&waker)),
            Poll::Pending
        ));
        gate.set(id);
        assert!(callback.observed_ready.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_replaced_waker_drop_can_reenter_without_holding_the_state_lock() {
        let gate = Arc::new(StagedCleanupIdentity::new());
        let observed_drop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed_wake = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let old_waker = Waker::from(Arc::new(ReentrantDrop {
            gate: Arc::clone(&gate),
            observed_drop: Arc::clone(&observed_drop),
            observed_wake: Arc::clone(&observed_wake),
        }));
        let mut future = Box::pin(gate.wait());
        assert!(matches!(
            future.as_mut().poll(&mut Context::from_waker(&old_waker)),
            Poll::Pending
        ));
        drop(old_waker);
        let new_waker = Waker::noop();
        assert!(matches!(
            future.as_mut().poll(&mut Context::from_waker(new_waker)),
            Poll::Pending
        ));
        assert!(observed_drop.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!observed_wake.load(std::sync::atomic::Ordering::SeqCst));
    }
}
