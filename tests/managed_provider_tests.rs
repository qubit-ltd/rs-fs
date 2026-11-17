// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "async")]
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;
#[cfg(feature = "async")]
use std::task::Wake;
use std::task::Waker;
use std::thread;
#[cfg(feature = "async")]
use std::thread::Thread;
use std::time::Duration;
use std::time::Instant;

use qubit_fs::FsEffectFact;
use qubit_fs::FsEffectReservation;
use qubit_fs::FsEffectSlotPool;
use qubit_fs::FsEffectSnapshot;
use qubit_fs::FsEffectState;
use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOperationFailure;
use qubit_fs::FsOwnedCopyRequest;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::FsOwnedRenameRequest;
use qubit_fs::FsOwnedTreePublishRequest;
use qubit_fs::FsOwnedWriteRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::FsPrepareFailureKind;
use qubit_fs::Path;
use qubit_fs::directory::TreePublishOutcome;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::spi::ManagedFsMutationSpi;
use qubit_fs::spi::ManagedFsReadSpi;
use qubit_fs::spi::ManagedFsTreeSpi;
use qubit_fs::spi::PreparedFsCopy;
use qubit_fs::spi::PreparedFsRead;
use qubit_fs::spi::PreparedFsRename;
use qubit_fs::spi::PreparedFsTreePublish;
use qubit_fs::spi::PreparedFsWrite;
use qubit_operation::AsyncCleanup;
use qubit_operation::ChildOutcomeKind;
use qubit_operation::ChildOutcomeSummary;
use qubit_operation::ChildRelease;
use qubit_operation::ChildSnapshot;
use qubit_operation::CleanupContext;
use qubit_operation::CleanupFuture;
use qubit_operation::CleanupOutcome;
use qubit_operation::DiagnosticSummary;
use qubit_operation::ExecutionFailure;
use qubit_operation::ExecutionPhase;
use qubit_operation::ExternalBindError;
use qubit_operation::GroupBindingError;
use qubit_operation::GroupOptions;
use qubit_operation::ManagerLimits;
use qubit_operation::ManagerOptions;
use qubit_operation::OperationControl;
use qubit_operation::OperationId;
use qubit_operation::OperationManager;
use qubit_operation::OperationOutcome;
use qubit_operation::OperationScope;
use qubit_operation::PreparedExternalOperation;
use qubit_operation::ResourceId;
use qubit_operation::ResourceRef;
use qubit_operation::ResourceReservation;
use qubit_operation::SettlementReport;
use qubit_operation::SettlementStatus;
use qubit_operation::ShutdownMode;
use qubit_operation::StopReason;
use qubit_operation::StopRequestStatus;
use qubit_operation::ThreadPoolExecutorOptions;

struct RejectReadProvider {
    prepared: AtomicUsize,
    io_started: AtomicUsize,
}
impl ManagedFsReadSpi for RejectReadProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("policy".into()),
            request,
        })
    }
}

#[test]
fn test_parent_scope_submission_preserves_request_when_prepare_rejects_without_io() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let provider = Arc::new(RejectReadProvider {
        prepared: AtomicUsize::new(0),
        io_started: AtomicUsize::new(0),
    });
    let path = Path::parse("/managed/input").unwrap();
    let expected = path.clone();
    let mut child_parent = manager
        .try_submit(Default::default(), {
            let provider = Arc::clone(&provider);
            move |scope| {
                let binding = FsExecutionBinding::new(scope.manager());
                let error = match binding.submit_read_all(
                    provider.as_ref(),
                    FsOwnedReadRequest {
                        path,
                        options: Default::default(),
                    },
                ) {
                    Ok(_) => panic!("provider rejected preparation"),
                    Err(error) => error,
                };
                let (_, preparation, request) = error.into_parts();
                assert_eq!(request.path, expected);
                assert_eq!(preparation, Some(FsPrepareFailureKind::Rejected("policy".into())));
                Ok::<_, ()>(())
            }
        })
        .unwrap();
    assert!(matches!(
        child_parent.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(provider.io_started.load(Ordering::SeqCst), 0);
}

#[test]
fn test_effect_slot_is_reserved_until_provider_releases_it() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("first slot is available");
    let observer = reservation.observer();
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(pool.reserve().is_err());

    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(reservation);
    assert!(pool.reserve().is_ok());
}

#[test]
fn test_dropping_observer_does_not_release_provider_resource_owner() {
    struct RecoveryOwner(FsEffectReservation);
    impl RecoveryOwner {
        fn finish_recovery(self) {
            drop(self.0);
        }
    }

    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("capacity available");
    let observer = reservation.observer();
    let owner = RecoveryOwner(reservation);
    drop(observer);
    assert!(pool.reserve().is_err(), "observer is not the resource owner");
    owner.finish_recovery();
    assert!(
        pool.reserve().is_ok(),
        "recovery owner releases its lease at completion"
    );
}

#[test]
fn test_managed_write_publishes_effect_and_settles_after_outcome_and_observer_drop() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let entered_tx;
    let entered_rx;
    (entered_tx, entered_rx) = mpsc::channel();
    let cleanup_released = Arc::new(AtomicBool::new(false));
    let resource_ids = Arc::new(Mutex::new(Vec::new()));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::clone(&cleanup_released),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::clone(&resource_ids),
        stop_requests: Arc::new(Mutex::new(Vec::new())),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = match binding.submit_write(&provider, write_request("/managed/write", b"payload"), &effects) {
        Ok(submitted) => submitted,
        Err(_) => panic!("write should be prepared and admitted"),
    };

    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("fake provider activated");
    assert!(matches!(submitted.effect.snapshot(), FsEffectSnapshot::Pending));
    assert!(effects.reserve().is_err(), "provider owner retains the reserved slot");
    gate.release();

    let mut operation = submitted.operation;
    let outcome = operation.wait_outcome().unwrap();
    assert!(
        matches!(outcome, OperationOutcome::Succeeded(ref value) if value.bytes_written() == Some(7)),
        "unexpected outcome: {outcome:?}"
    );
    assert!(matches!(
        submitted.effect.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(submitted.effect);
    assert!(!cleanup_released.load(Ordering::SeqCst));
    settlement_gate.release();
    let settlement = operation.wait_settlement().unwrap();
    assert!(settlement.all_released());
    assert!(cleanup_released.load(Ordering::SeqCst));
    let resource_ids = resource_ids.lock().unwrap();
    assert_eq!(resource_ids.len(), 1, "one physical write owner was registered");
    assert_eq!(
        settlement.resource_reports.len(),
        1,
        "the resource is counted once in settlement"
    );
    assert_eq!(settlement.resource_reports[0].resource_id, resource_ids[0]);
    assert!(effects.reserve().is_ok());
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
#[cfg(feature = "async")]
fn test_managed_async_write_uses_same_effect_and_outcome_contract() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(AtomicBool::new(false));
    let resource_ids = Arc::new(Mutex::new(Vec::new()));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released: Arc::clone(&cleanup_released),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids,
        stop_requests: Arc::new(Mutex::new(Vec::new())),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = match binding.submit_write_async(&provider, write_request("/managed/async", b"async"), &effects) {
        Ok(submitted) => submitted,
        Err(_) => panic!("async write should be prepared and admitted"),
    };
    let mut operation = submitted.operation;
    let outcome = block_on(operation.wait_outcome()).unwrap();
    assert!(matches!(outcome, OperationOutcome::Succeeded(value) if value.bytes_written() == Some(5)));
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(submitted.effect);
    settlement_gate.release();
    assert!(block_on(operation.wait_settlement()).all_released());
    assert!(cleanup_released.load(Ordering::SeqCst));
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn test_managed_write_rejections_return_request_and_release_fact_capacity() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeWriteProvider::new(FakeWriteMode::PrepareReject);
    let binding = FsExecutionBinding::new(manager.clone());
    let rejected = match binding.submit_write(&provider, write_request("/prepare/reject", b"owned"), &effects) {
        Ok(_) => panic!("prepare rejection expected"),
        Err(rejected) => rejected,
    };
    assert_eq!(rejected.request.path.as_str(), "/prepare/reject");
    assert_eq!(rejected.request.contents, b"owned");
    assert_eq!(
        rejected.preparation,
        Some(FsPrepareFailureKind::Rejected("prepare".into()))
    );
    assert!(effects.reserve().is_ok(), "prepare rejection returns the slot");

    let provider = FakeWriteProvider::new(FakeWriteMode::InstallReject);
    let rejected = match binding.submit_write(&provider, write_request("/install/reject", b"same"), &effects) {
        Ok(_) => panic!("reservation installation rejection expected"),
        Err(rejected) => rejected,
    };
    assert_eq!(rejected.request.path.as_str(), "/install/reject");
    assert_eq!(rejected.request.contents, b"same");
    assert_eq!(
        rejected.preparation,
        Some(FsPrepareFailureKind::ReservationInstallFailed)
    );
    assert!(effects.reserve().is_ok(), "install rejection returns the slot");
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[must_use]
fn write_request(path: &str, contents: &[u8]) -> FsOwnedWriteRequest {
    FsOwnedWriteRequest {
        path: Path::parse(path).unwrap(),
        contents: contents.to_vec(),
        options: Default::default(),
    }
}

enum FakeWriteMode {
    PrepareReject,
    InstallReject,
    Success {
        gate: Option<(Arc<FakeActivationGate>, mpsc::Sender<()>)>,
        cleanup_released: Arc<AtomicBool>,
        settlement_gate: Arc<FakeSettlementGate>,
        resource_ids: Arc<Mutex<Vec<ResourceId>>>,
        stop_requests: Arc<Mutex<Vec<StopReason>>>,
        prepared_probe: Arc<Mutex<Option<Arc<FakePreparedWrite>>>>,
    },
}

struct FakeWriteProvider(FakeWriteMode);
impl FakeWriteProvider {
    #[must_use]
    fn new(mode: FakeWriteMode) -> Self {
        Self(mode)
    }
}
impl ManagedFsMutationSpi for FakeWriteProvider {
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>> {
        match &self.0 {
            FakeWriteMode::PrepareReject => Err(FsPrepareFailure {
                kind: FsPrepareFailureKind::Rejected("prepare".into()),
                request,
            }),
            FakeWriteMode::InstallReject => Ok(Arc::new(FakePreparedWrite::new(
                request,
                true,
                None,
                Arc::new(AtomicBool::new(false)),
                Arc::new(FakeSettlementGate::default()),
                Arc::new(Mutex::new(Vec::new())),
                Arc::new(Mutex::new(Vec::new())),
            ))),
            FakeWriteMode::Success {
                gate,
                cleanup_released,
                settlement_gate,
                resource_ids,
                stop_requests,
                prepared_probe,
            } => {
                let prepared = Arc::new(FakePreparedWrite::new(
                    request,
                    false,
                    gate.clone(),
                    Arc::clone(cleanup_released),
                    Arc::clone(settlement_gate),
                    Arc::clone(resource_ids),
                    Arc::clone(stop_requests),
                ));
                *prepared_probe.lock().unwrap() = Some(Arc::clone(&prepared));
                Ok(prepared)
            }
        }
    }
    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>> {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Unsupported,
            request,
        })
    }
    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>> {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Unsupported,
            request,
        })
    }
}

struct FakePreparedWrite {
    request: Mutex<Option<FsOwnedWriteRequest>>,
    id: Mutex<Option<OperationId>>,
    reservation: Mutex<Option<FsEffectReservation>>,
    outcome: Mutex<Option<OperationOutcome<WriteOutcome, FsOperationFailure>>>,
    activated: AtomicBool,
    install_reject: bool,
    gate: Option<(Arc<FakeActivationGate>, mpsc::Sender<()>)>,
    cleanup_released: Arc<AtomicBool>,
    settlement_gate: Arc<FakeSettlementGate>,
    resource_ids: Arc<Mutex<Vec<ResourceId>>>,
    stop_requests: Arc<Mutex<Vec<StopReason>>>,
    resource_ref: Mutex<Option<ResourceRef<()>>>,
    resource_reservation: Mutex<Option<ResourceReservation>>,
}
impl FakePreparedWrite {
    #[must_use]
    fn new(
        request: FsOwnedWriteRequest,
        install_reject: bool,
        gate: Option<(Arc<FakeActivationGate>, mpsc::Sender<()>)>,
        cleanup_released: Arc<AtomicBool>,
        settlement_gate: Arc<FakeSettlementGate>,
        resource_ids: Arc<Mutex<Vec<ResourceId>>>,
        stop_requests: Arc<Mutex<Vec<StopReason>>>,
    ) -> Self {
        Self {
            request: Mutex::new(Some(request)),
            id: Mutex::new(None),
            reservation: Mutex::new(None),
            outcome: Mutex::new(None),
            activated: AtomicBool::new(false),
            install_reject,
            gate,
            cleanup_released,
            settlement_gate,
            resource_ids,
            stop_requests,
            resource_ref: Mutex::new(None),
            resource_reservation: Mutex::new(None),
        }
    }
    fn complete_success(&self) {
        if self.final_fact() {
            return;
        }
        let reservation = self
            .reservation
            .lock()
            .unwrap()
            .take()
            .expect("binding installs effect reservation before activation");
        if let Some(resource_reservation) = self.resource_reservation.lock().unwrap().take()
            && let Ok(resource) = resource_reservation.commit_with((), FakeWriteCleanup)
        {
            self.resource_ids.lock().unwrap().push(resource.id());
            *self.resource_ref.lock().unwrap() = Some(resource);
        }
        let fact = if self.stop_requests.lock().unwrap().is_empty() {
            FsEffectState::Applied
        } else {
            FsEffectState::Indeterminate
        };
        reservation.publish(FsEffectFact::State(fact));
        let bytes = self.request.lock().unwrap().as_ref().unwrap().contents.len() as u64;
        *self.outcome.lock().unwrap() = Some(OperationOutcome::Succeeded(
            WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct).with_bytes_written(bytes),
        ));
        self.activated.store(true, Ordering::SeqCst);
        drop(reservation);
    }
    #[must_use]
    fn final_fact(&self) -> bool {
        self.activated.load(Ordering::SeqCst)
    }
}
impl OperationControl for FakePreparedWrite {
    fn request_stop(&self, reason: StopReason) {
        self.stop_requests.lock().unwrap().push(reason);
    }
    fn snapshot(&self) -> ChildSnapshot {
        let operation_id = self.id.lock().unwrap().unwrap();
        let done = self.final_fact();
        ChildSnapshot {
            operation_id,
            execution: if done {
                ExecutionPhase::Finished
            } else {
                ExecutionPhase::Running
            },
            stop: None,
            outcome: done.then_some(ChildOutcomeSummary {
                operation_id,
                kind: ChildOutcomeKind::Succeeded,
                summary: DiagnosticSummary::new("child succeeded"),
            }),
            input_released: done,
            settlement: done.then(|| Arc::new(fake_settlement(operation_id))),
        }
    }
    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        if self.final_fact() {
            Poll::Ready(self.snapshot().outcome.unwrap())
        } else {
            Poll::Pending
        }
    }
    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        if self.final_fact() {
            Poll::Ready(ChildRelease {
                operation_id: self.id.lock().unwrap().unwrap(),
                confirmed_at: None,
            })
        } else {
            Poll::Pending
        }
    }
    fn poll_settlement(&self, cx: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        if self.final_fact() && self.settlement_gate.open.load(Ordering::SeqCst) {
            self.resource_ref.lock().unwrap().take();
            self.cleanup_released.store(true, Ordering::SeqCst);
            Poll::Ready(Arc::new(fake_settlement(self.id.lock().unwrap().unwrap())))
        } else {
            *self.settlement_gate.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl PreparedExternalOperation<WriteOutcome, FsOperationFailure> for FakePreparedWrite {
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        let mut bound = self.id.lock().unwrap();
        if bound.is_some() {
            return Err(ExternalBindError::AlreadyBound);
        }
        *bound = Some(id);
        Ok(())
    }
    fn activate(self: Arc<Self>, scope: OperationScope) -> Result<(), ExecutionFailure> {
        if let Ok(reservation) = scope.reserve_resource() {
            *self.resource_reservation.lock().unwrap() = Some(reservation);
        }
        if let Some((gate, entered)) = &self.gate {
            let _ = entered.send(());
            // Provider dispatch returns without blocking; the manager remains
            // free to forward stop requests while its work is pending.
            let _ = gate;
        } else {
            self.complete_success();
        }
        Ok(())
    }
    fn abandon(self: Arc<Self>) {
        if let Some(reservation) = self.reservation.lock().unwrap().take() {
            reservation.publish(FsEffectFact::State(FsEffectState::Unchanged));
        }
    }
    fn poll_take_outcome(&self, cx: &mut Context<'_>) -> Poll<OperationOutcome<WriteOutcome, FsOperationFailure>> {
        if let Some((gate, _)) = &self.gate {
            if !gate.poll_open(cx) {
                return Poll::Pending;
            }
            self.complete_success();
        }
        self.outcome.lock().unwrap().take().map_or(Poll::Pending, Poll::Ready)
    }
}

struct FakeWriteCleanup;
impl AsyncCleanup<()> for FakeWriteCleanup {
    fn cleanup(self: Arc<Self>, _: (), _: CleanupContext) -> CleanupFuture<()> {
        Box::pin(async { CleanupOutcome::Released })
    }
}
impl PreparedFsWrite for FakePreparedWrite {
    fn take_unclaimed_request(&self) -> Option<FsOwnedWriteRequest> {
        if self.id.lock().unwrap().is_some() {
            return None;
        }
        self.reservation.lock().unwrap().take();
        self.request.lock().unwrap().take()
    }
    fn install_effect_reservation(&self, reservation: FsEffectReservation) -> Result<(), FsEffectReservation> {
        if self.install_reject || self.reservation.lock().unwrap().is_some() {
            Err(reservation)
        } else {
            *self.reservation.lock().unwrap() = Some(reservation);
            Ok(())
        }
    }
}

fn fake_settlement(operation_id: OperationId) -> SettlementReport {
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

#[derive(Default)]
struct FakeActivationGate {
    open: AtomicBool,
    waker: Mutex<Option<Waker>>,
}
impl FakeActivationGate {
    #[must_use]
    fn poll_open(&self, cx: &Context<'_>) -> bool {
        if self.open.load(Ordering::SeqCst) {
            return true;
        }
        *self.waker.lock().unwrap() = Some(cx.waker().clone());
        self.open.load(Ordering::SeqCst)
    }
    fn release(&self) {
        self.open.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

#[derive(Default)]
struct FakeSettlementGate {
    open: AtomicBool,
    waker: Mutex<Option<Waker>>,
}
impl FakeSettlementGate {
    fn release(&self) {
        self.open.store(true, Ordering::SeqCst);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }
}

#[cfg(feature = "async")]
fn block_on<F: Future>(future: F) -> F::Output {
    struct ThreadWaker(Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[test]
fn test_manager_admission_rejection_recovers_write_request_and_reserved_slot() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(AtomicBool::new(false));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released,
        settlement_gate: Arc::new(FakeSettlementGate::default()),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::new(Mutex::new(Vec::new())),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager);
    let rejected = match binding.submit_write(&provider, write_request("/admission/reject", b"owned input"), &effects) {
        Ok(_) => panic!("closed manager must reject write"),
        Err(rejected) => rejected,
    };
    assert!(rejected.admission.is_some());
    assert!(rejected.preparation.is_none());
    assert_eq!(rejected.request.path.as_str(), "/admission/reject");
    assert_eq!(rejected.request.contents, b"owned input");
    assert!(effects.reserve().is_ok(), "unclaimed prepared owner released its slot");
}

#[test]
fn test_managed_tree_publish_uses_one_fact_slot_and_preserves_typed_publication_facts() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeTreeProvider;
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = binding
        .submit_tree_publish(&provider, tree_request(), &effects)
        .unwrap_or_else(|_| panic!("tree publication should be prepared"));
    let mut operation = submitted.operation;
    assert!(matches!(
        operation.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(outcome) if outcome.entries_published == 2 && outcome.content_bytes == 11
    ));
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::TreePublish { outcome, recovery: None })
            if outcome.entries_published == 2 && outcome.content_bytes == 11 && outcome.durable
    ));
    drop(submitted.effect);
    assert!(operation.wait_settlement().unwrap().all_released());
    assert!(effects.reserve().is_ok());
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[must_use]
fn tree_request() -> FsOwnedTreePublishRequest {
    FsOwnedTreePublishRequest {
        staging_root: Path::parse("/stage").unwrap(),
        destination_root: Path::parse("/destination").unwrap(),
        validation: Default::default(),
        validated_version: None,
        publication: Default::default(),
    }
}

struct FakeTreeProvider;
impl ManagedFsTreeSpi for FakeTreeProvider {
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>> {
        Ok(Arc::new(FakePreparedTree {
            request: Mutex::new(Some(request)),
            id: Mutex::new(None),
            reservation: Mutex::new(None),
            outcome: Mutex::new(None),
            completed: AtomicBool::new(false),
        }))
    }
}

struct FakePreparedTree {
    request: Mutex<Option<FsOwnedTreePublishRequest>>,
    id: Mutex<Option<OperationId>>,
    reservation: Mutex<Option<FsEffectReservation>>,
    outcome: Mutex<Option<OperationOutcome<TreePublishOutcome, FsOperationFailure>>>,
    completed: AtomicBool,
}
impl OperationControl for FakePreparedTree {
    fn request_stop(&self, _: StopReason) {}
    fn snapshot(&self) -> ChildSnapshot {
        let operation_id = self.id.lock().unwrap().unwrap();
        let done = self.completed.load(Ordering::SeqCst);
        ChildSnapshot {
            operation_id,
            execution: if done {
                ExecutionPhase::Finished
            } else {
                ExecutionPhase::Running
            },
            stop: None,
            outcome: done.then_some(ChildOutcomeSummary {
                operation_id,
                kind: ChildOutcomeKind::Succeeded,
                summary: DiagnosticSummary::new("child succeeded"),
            }),
            input_released: done,
            settlement: done.then(|| Arc::new(fake_settlement(operation_id))),
        }
    }
    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(self.snapshot().outcome.unwrap())
        } else {
            Poll::Pending
        }
    }
    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(ChildRelease {
                operation_id: self.id.lock().unwrap().unwrap(),
                confirmed_at: None,
            })
        } else {
            Poll::Pending
        }
    }
    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(Arc::new(fake_settlement(self.id.lock().unwrap().unwrap())))
        } else {
            Poll::Pending
        }
    }
}
impl PreparedExternalOperation<TreePublishOutcome, FsOperationFailure> for FakePreparedTree {
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        let mut bound = self.id.lock().unwrap();
        if bound.is_some() {
            return Err(ExternalBindError::AlreadyBound);
        }
        *bound = Some(id);
        Ok(())
    }
    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        let outcome = TreePublishOutcome {
            entries_published: 2,
            content_bytes: 11,
            atomicity: AchievedAtomicity::Atomic,
            method: PublicationMethod::AtomicRename,
            durable: true,
        };
        let reservation = self
            .reservation
            .lock()
            .unwrap()
            .take()
            .expect("binding installed effect reservation");
        reservation.publish_tree_outcome(outcome.clone(), None);
        *self.outcome.lock().unwrap() = Some(OperationOutcome::Succeeded(outcome));
        self.completed.store(true, Ordering::SeqCst);
        drop(reservation);
        Ok(())
    }
    fn abandon(self: Arc<Self>) {
        if let Some(reservation) = self.reservation.lock().unwrap().take() {
            reservation.publish(FsEffectFact::State(FsEffectState::Unchanged));
        }
    }
    fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<TreePublishOutcome, FsOperationFailure>> {
        self.outcome.lock().unwrap().take().map_or(Poll::Pending, Poll::Ready)
    }
}
impl PreparedFsTreePublish for FakePreparedTree {
    fn take_unclaimed_request(&self) -> Option<FsOwnedTreePublishRequest> {
        if self.id.lock().unwrap().is_some() {
            return None;
        }
        self.reservation.lock().unwrap().take();
        self.request.lock().unwrap().take()
    }
    fn install_effect_reservation(&self, reservation: FsEffectReservation) -> Result<(), FsEffectReservation> {
        let mut slot = self.reservation.lock().unwrap();
        if slot.is_some() {
            Err(reservation)
        } else {
            *slot = Some(reservation);
            Ok(())
        }
    }
}

#[test]
fn test_stop_control_remains_available_while_sync_executor_queue_is_saturated() {
    let options = ManagerOptions {
        limits: ManagerLimits {
            active_operations: 4,
            queued_operations: 8,
            admission_waiters: 8,
            ..Default::default()
        },
        sync_executor: ThreadPoolExecutorOptions {
            worker_count: 1,
            queue_capacity: 1,
        },
        ..Default::default()
    };
    let manager = OperationManager::new(options).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let (entered_tx, entered_rx) = mpsc::channel();
    let cleanup_released = Arc::new(AtomicBool::new(false));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let stop_requests = Arc::new(Mutex::new(Vec::new()));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released,
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let mut active = binding
        .submit_write(&provider, write_request("/queue/active", b"active"), &effects)
        .unwrap_or_else(|_| panic!("active managed write should be admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("the only executor worker entered provider activation");

    let (worker_entered_tx, worker_entered_rx) = mpsc::channel();
    let (worker_release_tx, worker_release_rx) = mpsc::channel();
    let mut blocking = manager
        .try_submit(Default::default(), move |_| {
            worker_entered_tx.send(()).expect("signal occupied executor worker");
            worker_release_rx.recv().expect("release occupied executor worker");
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|_| panic!("blocking executor task should be admitted"));
    worker_entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("the only executor worker entered the blocking task");

    let mut queued = manager
        .try_submit(Default::default(), |_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("single queue slot should accept one operation"));
    let saturated = manager.try_submit(Default::default(), |_| Ok::<_, ()>(()));
    assert!(saturated.is_err(), "executor queue is full");
    assert_eq!(
        active.operation.request_stop(StopReason::UserCancelled),
        StopRequestStatus::Accepted,
        "stop request is delivered through the handle without using the saturated work queue"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(
        *stop_requests.lock().unwrap(),
        vec![StopReason::UserCancelled],
        "the prepared provider receives the stop exactly once"
    );

    gate.release();
    worker_release_tx.send(()).expect("release executor worker");
    assert!(matches!(
        active.operation.wait_outcome().unwrap(),
        OperationOutcome::Stopped(_)
    ));
    assert!(matches!(
        active.effect.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
    drop(active.effect);
    settlement_gate.release();
    active.operation.wait_settlement().unwrap();
    assert!(matches!(
        queued.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    assert!(matches!(
        blocking.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn test_dropped_handle_keeps_pending_remote_owner_until_input_release_is_confirmed() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let (entered_tx, entered_rx) = mpsc::channel();
    let stop_requests = Arc::new(Mutex::new(Vec::new()));
    let prepared_probe = Arc::new(Mutex::new(None));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::new(AtomicBool::new(false)),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::clone(&prepared_probe),
    });
    let submitted = FsExecutionBinding::new(manager.clone())
        .submit_write(&provider, write_request("/remote/pending", b"owned"), &effects)
        .unwrap_or_else(|_| panic!("managed remote operation should be admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("provider activation returned while remote work is pending");
    let prepared = prepared_probe.lock().unwrap().clone().unwrap();
    assert!(!prepared.snapshot().input_released);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(prepared.poll_release(&mut context), Poll::Pending));
    assert!(matches!(prepared.poll_settlement(&mut context), Poll::Pending));
    assert!(matches!(submitted.effect.snapshot(), FsEffectSnapshot::Pending));
    assert!(
        effects.reserve().is_err(),
        "the running provider retains its owner and slot"
    );

    let observer = submitted.effect;
    drop(submitted.operation);
    let deadline = Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(*stop_requests.lock().unwrap(), vec![StopReason::UserCancelled]);
    assert!(!prepared.snapshot().input_released, "remote work still consumes input");
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(effects.reserve().is_err());

    gate.release();
    assert!(matches!(
        observer.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !prepared.snapshot().input_released && Instant::now() < deadline {
        thread::yield_now();
    }
    assert!(
        prepared.snapshot().input_released,
        "provider confirms it released the owned input"
    );
    assert!(matches!(prepared.poll_release(&mut context), Poll::Ready(_)));
    settlement_gate.release();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut reclaimed = false;
    while Instant::now() < deadline {
        if let Ok(reservation) = effects.reserve() {
            drop(reservation);
            reclaimed = true;
            break;
        }
        thread::yield_now();
    }
    assert!(
        reclaimed,
        "the fact slot returns after release and recovery responsibility ends"
    );
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn test_execution_binding_group_keeps_parent_target_for_children_after_group_close() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let group = manager.group(GroupOptions { capacity: 4 }).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(AtomicBool::new(false));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released,
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::new(Mutex::new(Vec::new())),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let mut parent = manager
        .try_submit(Default::default(), {
            let provider = provider;
            let effects = effects.clone();
            let group = group.clone();
            let settlement_gate = Arc::clone(&settlement_gate);
            move |scope| {
                let binding = FsExecutionBinding::new(scope.manager())
                    .with_group(&group)
                    .expect("group binds without replacing the parent target");
                let duplicate = binding
                    .with_group(&group)
                    .expect("repeated same group binding remains idempotent");
                assert!(duplicate.manager().with_group(&group).is_ok());
                group.close_admission();
                let submitted = binding
                    .submit_write(&provider, write_request("/group/child", b"group-owned"), &effects)
                    .unwrap_or_else(|_| panic!("an already-bound parent group admits its child"));
                let mut operation = submitted.operation;
                assert!(matches!(
                    operation.wait_outcome().unwrap(),
                    OperationOutcome::Succeeded(value) if value.bytes_written() == Some(11)
                ));
                assert!(matches!(
                    submitted.effect.wait_final(),
                    Ok(FsEffectFact::State(FsEffectState::Applied))
                ));
                drop(submitted.effect);
                settlement_gate.release();
                assert!(operation.wait_settlement().unwrap().all_released());
                Ok::<_, ()>(())
            }
        })
        .expect("parent operation accepted");
    assert!(matches!(
        parent.wait_outcome().unwrap(),
        OperationOutcome::Succeeded(())
    ));
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn test_execution_binding_rejects_wrong_manager_closed_and_over_limit_groups() {
    let manager = OperationManager::new(ManagerOptions {
        max_groups_per_operation: 1,
        ..Default::default()
    })
    .unwrap();
    let other = OperationManager::new(Default::default()).unwrap();
    let group_a = manager.group(GroupOptions { capacity: 4 }).unwrap();
    let group_b = manager.group(GroupOptions { capacity: 4 }).unwrap();
    let foreign_group = other.group(GroupOptions { capacity: 4 }).unwrap();
    let binding = FsExecutionBinding::new(manager.clone());
    assert!(matches!(
        binding.with_group(&foreign_group),
        Err(GroupBindingError::WrongManager)
    ));
    let bound = binding.with_group(&group_a).unwrap();
    assert!(matches!(
        bound.with_group(&group_b),
        Err(GroupBindingError::CapacityExceeded)
    ));
    group_a.close_admission();
    assert!(matches!(binding.with_group(&group_a), Err(GroupBindingError::Closed)));
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
    let _ = other.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[cfg(feature = "async")]
#[test]
fn test_async_binding_keeps_service_group_admission_constraints() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let group = manager.group(GroupOptions { capacity: 2 }).unwrap();
    let binding = FsExecutionBinding::new(manager.clone()).with_group(&group).unwrap();
    group.close_admission();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released: Arc::new(AtomicBool::new(false)),
        settlement_gate: Arc::new(FakeSettlementGate::default()),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::new(Mutex::new(Vec::new())),
        prepared_probe: Arc::new(Mutex::new(None)),
    });
    let error = match binding.submit_write_async(&provider, write_request("/async/group-closed", b"returned"), &effects)
    {
        Ok(_) => panic!("async manager view must retain the closed group boundary"),
        Err(error) => error,
    };
    assert!(error.admission.is_some());
    assert_eq!(error.request.path.as_str(), "/async/group-closed");
    assert_eq!(error.request.contents, b"returned");
    assert!(effects.reserve().is_ok());
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[cfg(feature = "async")]
#[test]
fn test_managed_async_tree_publish_preserves_typed_effect_facts() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let submitted = FsExecutionBinding::new(manager.clone())
        .submit_tree_publish_async(&FakeTreeProvider, tree_request(), &effects)
        .unwrap_or_else(|_| panic!("async tree publication should be admitted"));
    let mut operation = submitted.operation;
    let outcome = block_on(operation.wait_outcome()).unwrap();
    assert!(matches!(
        outcome,
        OperationOutcome::Succeeded(ref value)
            if value.entries_published == 2 && value.content_bytes == 11
    ));
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::TreePublish { outcome, recovery: None })
            if outcome.entries_published == 2 && outcome.durable
    ));
    drop(submitted.effect);
    assert!(block_on(operation.wait_settlement()).all_released());
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}

#[test]
fn test_unknown_remote_effect_with_released_input_can_settle_locally() {
    let manager = OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let (entered_tx, entered_rx) = mpsc::channel();
    let stop_requests = Arc::new(Mutex::new(Vec::new()));
    let prepared_probe = Arc::new(Mutex::new(None));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::new(AtomicBool::new(false)),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::clone(&prepared_probe),
    });
    let mut submitted = FsExecutionBinding::new(manager.clone())
        .submit_write(&provider, write_request("/remote/released", b"input"), &effects)
        .unwrap_or_else(|_| panic!("remote operation admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("prepared provider entered pending remote work");
    let prepared = prepared_probe.lock().unwrap().clone().unwrap();
    assert!(!prepared.snapshot().input_released);
    assert_eq!(
        submitted.operation.request_stop(StopReason::UserCancelled),
        StopRequestStatus::Accepted
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(*stop_requests.lock().unwrap(), vec![StopReason::UserCancelled]);
    gate.release();
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
    let deadline = Instant::now() + Duration::from_secs(3);
    while !prepared.snapshot().input_released && Instant::now() < deadline {
        thread::yield_now();
    }
    assert!(prepared.snapshot().input_released);
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(prepared.poll_release(&mut context), Poll::Ready(_)));
    settlement_gate.release();
    assert!(matches!(
        submitted.operation.wait_outcome().unwrap(),
        OperationOutcome::Stopped(_)
    ));
    assert!(submitted.operation.wait_settlement().unwrap().all_released());
    assert!(
        effects.reserve().is_ok(),
        "released local input permits settlement with unknown remote effect"
    );
    let _ = manager.request_shutdown(ShutdownMode::Drain).wait().unwrap();
}
