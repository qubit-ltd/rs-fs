// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Integration contracts for managed operation admission, request ownership,
//! effect observation, and prepared-provider lifecycle transitions.

use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use qubit_fs::AsyncFsOperation;
use qubit_fs::FsEffectFact;
use qubit_fs::FsEffectSlotPool;
use qubit_fs::FsEffectSnapshot;
use qubit_fs::FsEffectState;
use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOperation;
use qubit_fs::FsOperationFailure;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::FsSubmitFailure;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::ReadConsistencyPolicy;
use qubit_fs::execution::FsPrepareFailureKind;
use qubit_fs::execution::PreparedFsReadAdapter;
use qubit_fs::spi::ManagedFsReadSpi;
use qubit_fs::spi::PreparedFsRead;
use qubit_operation::AdmissionError;
use qubit_operation::ChildOutcomeKind;
use qubit_operation::ChildOutcomeSummary;
use qubit_operation::ChildRelease;
use qubit_operation::ChildSnapshot;
use qubit_operation::DiagnosticSummary;
use qubit_operation::ExecutionFailure;
use qubit_operation::ExecutionPhase;
use qubit_operation::ExternalBindError;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationControl;
use qubit_operation::OperationId;
use qubit_operation::OperationManager;
use qubit_operation::OperationOptions;
use qubit_operation::OperationOutcome;
use qubit_operation::OperationScope;
use qubit_operation::PreparedExternalOperation;
use qubit_operation::SettlementReport;
use qubit_operation::SettlementStatus;
use qubit_operation::ShutdownMode;
use qubit_operation::StopReason;

#[test]
fn test_operation_aliases_are_public() {
    let _: Option<FsOperation<Vec<u8>>> = None;
    let _: Option<AsyncFsOperation<Vec<u8>>> = None;
}

#[test]
fn test_common_owned_contract_is_exported_and_object_safe() {
    fn accepts_provider(_: &dyn ManagedFsReadSpi) {}
    let _ = accepts_provider;
    let _: Option<FsOperationFailure> = None;
    let _: Option<FsPrepareFailure<FsOwnedReadRequest>> = None;
    let _: Option<FsSubmitFailure<FsOwnedReadRequest>> = None;
    let _: Option<Arc<dyn PreparedFsRead>> = None;
    let request = FsOwnedReadRequest {
        path: Path::parse("/data").unwrap(),
        options: ReadAllOptions::default(),
    };
    assert_eq!(request.path.as_str(), "/data");
}

#[test]
fn test_preparation_failure_returns_the_original_request() {
    let request = FsOwnedReadRequest {
        path: Path::parse("/same-request").unwrap(),
        options: ReadAllOptions::new(7, ReadConsistencyPolicy::ObserveChanges),
    };
    let failure = FsPrepareFailure {
        kind: FsPrepareFailureKind::Unsupported,
        request,
    };
    let (kind, recovered) = failure.into_parts();
    assert_eq!(kind, FsPrepareFailureKind::Unsupported);
    assert_eq!(recovered.path.as_str(), "/same-request");
    assert_eq!(recovered.options.max_bytes, 7);
}

#[test]
fn test_managed_submission_rejection_returns_the_owned_request() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let shutdown = manager.request_shutdown(ShutdownMode::Drain);
    let _report = shutdown.wait().unwrap();
    let binding = FsExecutionBinding::new(manager);
    let request = FsOwnedReadRequest {
        path: Path::parse("/admission-reject").unwrap(),
        options: ReadAllOptions::default(),
    };
    let failure = match binding.submit_read_all(&HoldingProvider, request) {
        Ok(_) => panic!("closed manager unexpectedly accepted submission"),
        Err(failure) => failure,
    };
    let (reason, preparation, recovered) = failure.into_parts();
    assert!(reason.is_some(), "expected manager admission error: {reason:?}");
    assert!(preparation.is_none());
    assert_eq!(recovered.path.as_str(), "/admission-reject");
}

#[test]
fn test_effect_fact_slots_are_bounded_reclaimed_and_final_is_immutable() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let clone = reservation.observer();
    let observer = clone.clone();
    assert!(pool.reserve().is_err());
    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    clone.publish(FsEffectState::Indeterminate);
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(clone);
    drop(observer);
    assert!(
        pool.reserve().is_err(),
        "active work retains its reservation after observers are dropped"
    );
    drop(reservation);
    assert!(pool.reserve().is_ok());
}

/// Returns a prepared read that keeps its request available for rejection
/// tests.
struct HoldingProvider;
impl ManagedFsReadSpi for HoldingProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        Ok(Arc::new(HoldingRead {
            request: Mutex::new(Some(request)),
        }))
    }
}

/// Prepared read fixture that remains pending and retains the original request.
struct HoldingRead {
    request: Mutex<Option<FsOwnedReadRequest>>,
}
impl OperationControl for HoldingRead {
    fn request_stop(&self, _: StopReason) {}
    fn snapshot(&self) -> ChildSnapshot {
        unimplemented!("closed-manager rejection never observes the prepared child")
    }
    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        Poll::Pending
    }
    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        Poll::Pending
    }
    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        Poll::Pending
    }
}
impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for HoldingRead {
    fn bind(&self, _: OperationId) -> Result<(), ExternalBindError> {
        Ok(())
    }
    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        Ok(())
    }
    fn abandon(self: Arc<Self>) {}
    fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        Poll::Pending
    }
}
impl PreparedFsRead for HoldingRead {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.request.lock().expect("request mutex not poisoned").take()
    }
}

#[test]
fn test_effect_observer_waits_for_final_facts_in_sync_and_async_paths() {
    use std::future::Future;

    let pool = FsEffectSlotPool::new(2);
    let sync_reservation = pool.reserve().unwrap();
    let sync_observer = sync_reservation.observer();
    sync_reservation.publish(FsEffectFact::State(FsEffectState::Unchanged));
    assert!(matches!(
        sync_observer.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Unchanged))
    ));

    let async_reservation = pool.reserve().unwrap();
    let async_observer = async_reservation.observer();
    let mut wait = Box::pin(async_observer.wait_final_async());
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(wait.as_mut().poll(&mut context), Poll::Pending));
    async_reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        wait.as_mut().poll(&mut context),
        Poll::Ready(Ok(FsEffectFact::State(FsEffectState::Applied)))
    ));
}

#[test]
fn test_request_is_recoverable_only_while_unclaimed() {
    let (adapter, counters) = lifecycle_adapter("/recover", false, None);
    let request = adapter.take_request().expect("unclaimed request is recoverable");
    assert_eq!(request.path.as_str(), "/recover");
    assert_eq!(counters.unclaimed_request_reads.load(Ordering::SeqCst), 1);

    let id = fresh_operation_id();
    let (bound, counters) = lifecycle_adapter("/bound", false, None);
    PreparedExternalOperation::bind(bound.as_ref(), id).unwrap();
    assert!(bound.take_request().is_none());
    assert_eq!(counters.unclaimed_request_reads.load(Ordering::SeqCst), 0);
    assert_eq!(counters.abandons.load(Ordering::SeqCst), 0);
}

#[test]
fn test_post_bind_pre_activation_submit_error_preserves_existing_claim() {
    let claim_manager_id = fresh_operation_id();
    let second_manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let (adapter, counters) = lifecycle_adapter("/cross-manager", false, None);
    PreparedExternalOperation::bind(adapter.as_ref(), claim_manager_id).unwrap();

    let rejected = second_manager
        .try_submit_external::<Vec<u8>, FsOperationFailure, _>(OperationOptions::default(), Arc::clone(&adapter));
    let (reason, returned) = match rejected {
        Ok(_) => panic!("a prepared task was claimed by two manager submissions"),
        Err(error) => error.into_parts(),
    };
    assert_eq!(reason, AdmissionError::AlreadyBound);
    assert!(returned.take_request().is_none());
    assert_eq!(counters.binds.load(Ordering::SeqCst), 1);
    assert_eq!(counters.activations.load(Ordering::SeqCst), 0);
    assert_eq!(counters.abandons.load(Ordering::SeqCst), 0);
}

#[test]
fn test_bound_but_not_activated_task_is_abandoned_exactly_once() {
    let id = fresh_operation_id();
    let (adapter, counters) = lifecycle_adapter("/abandon-before-activate", false, None);
    PreparedExternalOperation::bind(adapter.as_ref(), id).unwrap();
    Arc::clone(&adapter).abandon();
    Arc::clone(&adapter).abandon();
    assert_eq!(counters.binds.load(Ordering::SeqCst), 1);
    assert_eq!(counters.activations.load(Ordering::SeqCst), 0);
    assert_eq!(counters.abandons.load(Ordering::SeqCst), 1);
}

#[test]
fn test_activation_failure_never_falls_back_to_abandon() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let (adapter, counters) = lifecycle_adapter("/activate-fails", true, None);
    let mut operation = manager
        .try_submit_external::<Vec<u8>, FsOperationFailure, _>(OperationOptions::default(), Arc::clone(&adapter))
        .unwrap();
    assert!(matches!(
        operation.wait_outcome().unwrap(),
        OperationOutcome::ExecutionFailed(_)
    ));
    Arc::clone(&adapter).abandon();
    assert_eq!(counters.activations.load(Ordering::SeqCst), 1);
    assert_eq!(counters.abandons.load(Ordering::SeqCst), 0);
}

#[test]
fn test_drain_during_gated_bind_activates_the_claimed_operation() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let binding = FsExecutionBinding::new(manager.clone());
    let gate = Arc::new(BindGate::new());
    let counters = Arc::new(LifecycleCounters::default());
    let provider = GatedProvider {
        gate: Arc::clone(&gate),
        counters: Arc::clone(&counters),
    };
    let submit = std::thread::spawn(move || {
        binding.submit_read_all(
            &provider,
            FsOwnedReadRequest {
                path: Path::parse("/claimed-during-drain").unwrap(),
                options: ReadAllOptions::default(),
            },
        )
    });
    gate.entered.wait();
    let shutdown = manager.request_shutdown(ShutdownMode::Drain);
    gate.release.wait();
    let mut operation = submit
        .join()
        .unwrap()
        .expect("a successful claim returns a handle during drain");
    let outcome = operation.wait_outcome().unwrap();
    assert!(
        matches!(outcome, OperationOutcome::Succeeded(ref value) if value.as_slice() == [9]),
        "outcome: {outcome:?}"
    );
    assert_eq!(counters.binds.load(Ordering::SeqCst), 1);
    assert_eq!(counters.activations.load(Ordering::SeqCst), 1);
    assert_eq!(counters.abandons.load(Ordering::SeqCst), 0);
    assert_eq!(counters.unclaimed_request_reads.load(Ordering::SeqCst), 0);
    let _report = shutdown.wait().unwrap();
}

/// Counts the binding and lifecycle callbacks observed by the prepared fixture.
#[derive(Default)]
struct LifecycleCounters {
    binds: AtomicUsize,
    activations: AtomicUsize,
    abandons: AtomicUsize,
    unclaimed_request_reads: AtomicUsize,
}
/// Pauses provider binding so a test can request manager shutdown mid-submit.
struct BindGate {
    entered: Barrier,
    release: Barrier,
}
impl BindGate {
    /// Creates two-party barriers for the submitter and coordinating test.
    fn new() -> Self {
        Self {
            entered: Barrier::new(2),
            release: Barrier::new(2),
        }
    }
}
/// Provider fixture that returns a prepared read whose bind callback is gated.
struct GatedProvider {
    gate: Arc<BindGate>,
    counters: Arc<LifecycleCounters>,
}
impl ManagedFsReadSpi for GatedProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        Ok(Arc::new(LifecyclePrepared::new(
            request,
            Arc::clone(&self.counters),
            false,
            Some(Arc::clone(&self.gate)),
        )))
    }
}
/// Builds a read adapter and counters with configurable activation and bind
/// behavior.
fn lifecycle_adapter(
    path: &str,
    activation_fails: bool,
    gate: Option<Arc<BindGate>>,
) -> (Arc<PreparedFsReadAdapter>, Arc<LifecycleCounters>) {
    let counters = Arc::new(LifecycleCounters::default());
    let request = FsOwnedReadRequest {
        path: Path::parse(path).unwrap(),
        options: ReadAllOptions::default(),
    };
    let prepared: Arc<dyn PreparedFsRead> = Arc::new(LifecyclePrepared::new(
        request,
        Arc::clone(&counters),
        activation_fails,
        gate,
    ));
    (Arc::new(PreparedFsReadAdapter::new(prepared)), counters)
}
/// Simulates a prepared read through binding, activation, outcome, and
/// abandonment.
struct LifecyclePrepared {
    request: Mutex<Option<FsOwnedReadRequest>>,
    operation_id: Mutex<Option<OperationId>>,
    outcome: Mutex<Option<OperationOutcome<Vec<u8>, FsOperationFailure>>>,
    completed: AtomicBool,
    abandoned: AtomicBool,
    counters: Arc<LifecycleCounters>,
    activation_fails: bool,
    gate: Option<Arc<BindGate>>,
}
impl LifecyclePrepared {
    /// Initializes an unbound prepared read with optional failure and bind
    /// gating.
    fn new(
        request: FsOwnedReadRequest,
        counters: Arc<LifecycleCounters>,
        activation_fails: bool,
        gate: Option<Arc<BindGate>>,
    ) -> Self {
        Self {
            request: Mutex::new(Some(request)),
            operation_id: Mutex::new(None),
            outcome: Mutex::new(None),
            completed: AtomicBool::new(false),
            abandoned: AtomicBool::new(false),
            counters,
            activation_fails,
            gate,
        }
    }
}
impl OperationControl for LifecyclePrepared {
    fn request_stop(&self, _: StopReason) {}
    fn snapshot(&self) -> ChildSnapshot {
        let operation_id = self.operation_id.lock().unwrap().expect("binding precedes observation");
        let completed = self.completed.load(Ordering::SeqCst);
        let abandoned = self.abandoned.load(Ordering::SeqCst);
        let final_fact = completed || abandoned;
        let kind = if abandoned {
            ChildOutcomeKind::Stopped
        } else {
            ChildOutcomeKind::Succeeded
        };
        let summary = if abandoned { "child stopped" } else { "child succeeded" };
        ChildSnapshot {
            operation_id,
            execution: if final_fact {
                ExecutionPhase::Finished
            } else {
                ExecutionPhase::Running
            },
            stop: None,
            outcome: final_fact.then(|| ChildOutcomeSummary {
                operation_id,
                kind,
                summary: DiagnosticSummary::new(summary),
            }),
            input_released: final_fact,
            settlement: final_fact.then(|| Arc::new(empty_settlement(operation_id))),
        }
    }
    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        let operation_id = self.operation_id.lock().unwrap().expect("binding precedes observation");
        if self.completed.load(Ordering::SeqCst) || self.abandoned.load(Ordering::SeqCst) {
            let stopped = self.abandoned.load(Ordering::SeqCst);
            let kind = if stopped {
                ChildOutcomeKind::Stopped
            } else {
                ChildOutcomeKind::Succeeded
            };
            let summary = if stopped { "child stopped" } else { "child succeeded" };
            Poll::Ready(ChildOutcomeSummary {
                operation_id,
                kind,
                summary: DiagnosticSummary::new(summary),
            })
        } else {
            Poll::Pending
        }
    }
    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        let operation_id = self.operation_id.lock().unwrap().expect("binding precedes observation");
        if self.completed.load(Ordering::SeqCst) || self.abandoned.load(Ordering::SeqCst) {
            Poll::Ready(ChildRelease {
                operation_id,
                confirmed_at: None,
            })
        } else {
            Poll::Pending
        }
    }
    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        let operation_id = self.operation_id.lock().unwrap().expect("binding precedes observation");
        if self.completed.load(Ordering::SeqCst) || self.abandoned.load(Ordering::SeqCst) {
            Poll::Ready(Arc::new(empty_settlement(operation_id)))
        } else {
            Poll::Pending
        }
    }
}
impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for LifecyclePrepared {
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        self.counters.binds.fetch_add(1, Ordering::SeqCst);
        *self.operation_id.lock().unwrap() = Some(id);
        if let Some(gate) = &self.gate {
            gate.entered.wait();
            gate.release.wait();
        }
        Ok(())
    }
    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        self.counters.activations.fetch_add(1, Ordering::SeqCst);
        if self.activation_fails {
            let operation_id = self.operation_id.lock().unwrap().expect("bind precedes activation");
            return Err(ExecutionFailure {
                operation_id,
                summary: DiagnosticSummary::new("activation rejected"),
                source: None,
            });
        }
        *self.outcome.lock().unwrap() = Some(OperationOutcome::Succeeded(vec![9]));
        self.completed.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn abandon(self: Arc<Self>) {
        self.counters.abandons.fetch_add(1, Ordering::SeqCst);
        self.abandoned.store(true, Ordering::SeqCst);
    }
    fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        self.outcome.lock().unwrap().take().map_or(Poll::Pending, Poll::Ready)
    }
}
impl PreparedFsRead for LifecyclePrepared {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.counters.unclaimed_request_reads.fetch_add(1, Ordering::SeqCst);
        self.request.lock().unwrap().take()
    }
}

/// Creates an empty released report for the local prepared-operation fixture.
fn empty_settlement(operation_id: OperationId) -> SettlementReport {
    SettlementReport {
        operation_id,
        status: SettlementStatus::Released,
        released_count: 0,
        transferred_count: 0,
        recovery_count: 0,
        children: Arc::from([]),
        children_total: 0,
        children_truncated: false,
        cleanup_failures: Arc::from([]),
        cleanup_failures_total: 0,
        cleanup_failures_truncated: false,
        resource_reports: Arc::from([]),
        resource_reports_total: 0,
        resource_reports_truncated: false,
        transfers: Arc::from([]),
        transfers_total: 0,
        transfers_truncated: false,
        recoveries: Arc::from([]),
        recoveries_total: 0,
        recoveries_truncated: false,
        effects: Arc::from([]),
        effects_total: 0,
        effect_overflow: 0,
        external_settlement: None,
    }
}

/// Obtains an operation ID by submitting and completing a minimal root
/// operation.
fn fresh_operation_id() -> OperationId {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let mut operation = manager
        .try_submit(OperationOptions::default(), |_| Ok::<(), ()>(()))
        .unwrap();
    let id = operation.id();
    assert!(matches!(
        operation.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    let _shutdown = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
    id
}
