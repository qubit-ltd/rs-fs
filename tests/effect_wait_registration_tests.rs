// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::future::Future;
use std::panic::panic_any;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Weak;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;

use qubit_fs::FsEffectFact;
use qubit_fs::FsEffectObservationError;
use qubit_fs::FsEffectSlotPool;
use qubit_fs::FsEffectSnapshot;
use qubit_fs::FsEffectState;

struct CountingWake(Arc<AtomicUsize>);

impl Wake for CountingWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanickingWake;

struct PanickingPayload;

impl Drop for PanickingPayload {
    fn drop(&mut self) {
        panic_any(SecondPanickingPayload);
    }
}

struct SecondPanickingPayload;

impl Drop for SecondPanickingPayload {
    fn drop(&mut self) {
        panic!("test second panic payload destructor panic");
    }
}

impl Wake for PanickingWake {
    fn wake(self: Arc<Self>) {
        panic_any(PanickingPayload);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        panic_any(PanickingPayload);
    }
}

fn counting_waker(counter: &Arc<AtomicUsize>) -> Waker {
    Waker::from(Arc::new(CountingWake(Arc::clone(counter))))
}

fn poll_pending<F: Future>(future: &mut Pin<Box<F>>, waker: &Waker) {
    let mut context = Context::from_waker(waker);
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
}

#[test]
fn test_effect_wait_registration_repeated_poll_wakes_once() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    let counter = Arc::new(AtomicUsize::new(0));
    let waker = counting_waker(&counter);
    let mut future = Box::pin(observer.wait_final_async());

    poll_pending(&mut future, &waker);
    poll_pending(&mut future, &waker);
    reservation.publish(FsEffectFact::State(FsEffectState::Applied));

    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_effect_wait_registration_replaces_waker_and_releases_old_task() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    let counter = Arc::new(AtomicUsize::new(0));
    let first = Arc::new(CountingWake(Arc::clone(&counter)));
    let first_weak = Arc::downgrade(&first);
    let first_waker = Waker::from(Arc::clone(&first));
    let mut future = Box::pin(observer.wait_final_async());

    poll_pending(&mut future, &first_waker);
    drop(first);
    drop(first_waker);
    assert!(first_weak.upgrade().is_some());

    let second = counting_waker(&counter);
    poll_pending(&mut future, &second);
    assert!(first_weak.upgrade().is_none());
    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    drop(future);
}

#[test]
fn test_effect_wait_registration_cancellation_releases_waker_and_keeps_other_waiter() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    let counter = Arc::new(AtomicUsize::new(0));
    let cancelled = Arc::new(CountingWake(Arc::clone(&counter)));
    let cancelled_weak: Weak<CountingWake> = Arc::downgrade(&cancelled);
    let cancelled_waker = Waker::from(Arc::clone(&cancelled));
    let active_waker = counting_waker(&counter);
    let mut cancelled_future = Box::pin(observer.wait_final_async());
    let mut active_future = Box::pin(observer.wait_final_async());

    poll_pending(&mut cancelled_future, &cancelled_waker);
    poll_pending(&mut active_future, &active_waker);
    drop(cancelled);
    drop(cancelled_waker);
    drop(cancelled_future);
    assert!(cancelled_weak.upgrade().is_none());

    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    drop(active_future);
}

#[test]
fn test_effect_wait_registration_keeps_futures_with_same_waker_independent() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    let first_counter = Arc::new(AtomicUsize::new(0));
    let second_counter = Arc::new(AtomicUsize::new(0));
    let first_waker = counting_waker(&first_counter);
    let mut first = Box::pin(observer.wait_final_async());
    let mut second = Box::pin(observer.wait_final_async());

    poll_pending(&mut first, &first_waker);
    poll_pending(&mut second, &first_waker);
    drop(first);
    let second_waker = counting_waker(&second_counter);
    poll_pending(&mut second, &second_waker);
    reservation.publish(FsEffectFact::State(FsEffectState::Applied));

    assert_eq!(first_counter.load(Ordering::SeqCst), 0);
    assert_eq!(second_counter.load(Ordering::SeqCst), 1);
    drop(second);
}

#[test]
fn test_effect_wait_registration_final_snapshot_does_not_retain_waker() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    let counter = Arc::new(AtomicUsize::new(0));
    let wake = Arc::new(CountingWake(counter));
    let weak = Arc::downgrade(&wake);
    let waker = Waker::from(Arc::clone(&wake));
    let mut future = Box::pin(observer.wait_final_async());
    {
        let mut context = Context::from_waker(&waker);
        assert!(matches!(future.as_mut().poll(&mut context), Poll::Ready(Ok(_))));
    }
    drop(waker);
    drop(wake);
    assert!(weak.upgrade().is_none());
}

#[test]
fn test_effect_wait_registration_last_reservation_closes_and_wakes_all_waiters() {
    let pool = FsEffectSlotPool::new(1);
    let first_reservation = pool.reserve().unwrap();
    let last_reservation = first_reservation.clone();
    let observer = first_reservation.observer();
    let first_counter = Arc::new(AtomicUsize::new(0));
    let second_counter = Arc::new(AtomicUsize::new(0));
    let first_waker = counting_waker(&first_counter);
    let second_waker = counting_waker(&second_counter);
    let mut first = Box::pin(observer.wait_final_async());
    let mut second = Box::pin(observer.wait_final_async());

    poll_pending(&mut first, &first_waker);
    poll_pending(&mut second, &second_waker);
    drop(first_reservation);
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(pool.reserve().is_err());
    drop(last_reservation);

    assert_eq!(first_counter.load(Ordering::SeqCst), 1);
    assert_eq!(second_counter.load(Ordering::SeqCst), 1);
    for future in [&mut first, &mut second] {
        let mut context = Context::from_waker(&first_waker);
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(FsEffectObservationError::PublisherClosed))
        ));
    }
    assert!(pool.reserve().is_ok());
}

#[test]
fn test_effect_wait_registration_panicking_waker_does_not_block_close_or_capacity() {
    let pool = FsEffectSlotPool::new(1);
    let first_reservation = pool.reserve().unwrap();
    let last_reservation = first_reservation.clone();
    let observer = first_reservation.observer();
    let healthy_counter = Arc::new(AtomicUsize::new(0));
    let panicking_waker = Waker::from(Arc::new(PanickingWake));
    let healthy_waker = counting_waker(&healthy_counter);
    let mut panicking_future = Box::pin(observer.wait_final_async());
    let mut healthy_future = Box::pin(observer.wait_final_async());

    poll_pending(&mut panicking_future, &panicking_waker);
    poll_pending(&mut healthy_future, &healthy_waker);
    drop(first_reservation);
    drop(last_reservation);

    assert_eq!(healthy_counter.load(Ordering::SeqCst), 1);
    assert!(pool.reserve().is_ok());
    for future in [&mut panicking_future, &mut healthy_future] {
        let mut context = Context::from_waker(&healthy_waker);
        assert!(matches!(
            future.as_mut().poll(&mut context),
            Poll::Ready(Err(FsEffectObservationError::PublisherClosed))
        ));
    }
}

#[test]
fn test_effect_wait_registration_panicking_payload_does_not_block_publish_or_close() {
    for publish in [true, false] {
        let pool = FsEffectSlotPool::new(1);
        let first_reservation = pool.reserve().unwrap();
        let last_reservation = first_reservation.clone();
        let observer = first_reservation.observer();
        let healthy_counter = Arc::new(AtomicUsize::new(0));
        let panicking_waker = Waker::from(Arc::new(PanickingWake));
        let healthy_waker = counting_waker(&healthy_counter);
        let mut panicking_future = Box::pin(observer.wait_final_async());
        let mut healthy_future = Box::pin(observer.wait_final_async());

        poll_pending(&mut panicking_future, &panicking_waker);
        poll_pending(&mut healthy_future, &healthy_waker);
        if publish {
            first_reservation.publish(FsEffectFact::State(FsEffectState::Applied));
            drop(first_reservation);
            drop(last_reservation);
            assert!(matches!(observer.snapshot(), FsEffectSnapshot::Final(_)));
        } else {
            drop(first_reservation);
            drop(last_reservation);
            assert!(matches!(observer.snapshot(), FsEffectSnapshot::Closed));
        }

        assert_eq!(1, healthy_counter.load(Ordering::SeqCst));
        assert!(pool.reserve().is_ok());
        drop(panicking_future);
        drop(healthy_future);
    }
}
