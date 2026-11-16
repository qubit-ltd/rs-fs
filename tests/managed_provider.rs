use qubit_fs::{
    FsEffectFact, FsEffectSlotPool, FsEffectSnapshot, FsEffectState, FsExecutionBinding,
    FsOwnedReadRequest, FsPrepareFailure, FsPrepareFailureKind, Path,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct RejectReadProvider {
    prepared: AtomicUsize,
    io_started: AtomicUsize,
}
impl qubit_fs::spi::ManagedFsReadSpi for RejectReadProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("policy".into()),
            request,
        })
    }
}

#[test]
fn parent_scope_submission_preserves_request_when_prepare_rejects_without_io() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
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
                assert_eq!(
                    preparation,
                    Some(FsPrepareFailureKind::Rejected("policy".into()))
                );
                Ok::<_, ()>(())
            }
        })
        .unwrap();
    assert!(matches!(
        child_parent.wait_outcome().unwrap(),
        qubit_operation::OperationOutcome::Succeeded(())
    ));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(provider.io_started.load(Ordering::SeqCst), 0);
}

#[test]
fn effect_slot_is_reserved_until_provider_releases_it() {
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
fn dropping_observer_does_not_release_provider_resource_owner() {
    struct RecoveryOwner(qubit_fs::FsEffectReservation);
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
    assert!(
        pool.reserve().is_err(),
        "observer is not the resource owner"
    );
    owner.finish_recovery();
    assert!(
        pool.reserve().is_ok(),
        "recovery owner releases its lease at completion"
    );
}

#[test]
fn managed_write_publishes_effect_and_settles_after_outcome_and_observer_drop() {
    use std::time::Duration;

    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let entered_tx;
    let entered_rx;
    (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let cleanup_released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let resource_ids = Arc::new(std::sync::Mutex::new(Vec::new()));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::clone(&cleanup_released),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::clone(&resource_ids),
        stop_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = match binding.submit_write(
        &provider,
        write_request("/managed/write", b"payload"),
        &effects,
    ) {
        Ok(submitted) => submitted,
        Err(_) => panic!("write should be prepared and admitted"),
    };

    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("fake provider activated");
    assert!(matches!(
        submitted.effect.snapshot(),
        FsEffectSnapshot::Pending
    ));
    assert!(
        effects.reserve().is_err(),
        "provider owner retains the reserved slot"
    );
    gate.release();

    let mut operation = submitted.operation;
    let outcome = operation.wait_outcome().unwrap();
    assert!(
        matches!(outcome, qubit_operation::OperationOutcome::Succeeded(ref value) if value.bytes_written() == Some(7)),
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
    assert_eq!(
        resource_ids.len(),
        1,
        "one physical write owner was registered"
    );
    assert_eq!(
        settlement.resource_reports.len(),
        1,
        "the resource is counted once in settlement"
    );
    assert_eq!(settlement.resource_reports[0].resource_id, resource_ids[0]);
    assert!(effects.reserve().is_ok());
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
#[cfg(feature = "async")]
fn managed_async_write_uses_same_effect_and_outcome_contract() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let resource_ids = Arc::new(std::sync::Mutex::new(Vec::new()));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released: Arc::clone(&cleanup_released),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids,
        stop_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = match binding.submit_write_async(
        &provider,
        write_request("/managed/async", b"async"),
        &effects,
    ) {
        Ok(submitted) => submitted,
        Err(_) => panic!("async write should be prepared and admitted"),
    };
    let mut operation = submitted.operation;
    let outcome = block_on(operation.wait_outcome()).unwrap();
    assert!(
        matches!(outcome, qubit_operation::OperationOutcome::Succeeded(value) if value.bytes_written() == Some(5))
    );
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(submitted.effect);
    settlement_gate.release();
    assert!(block_on(operation.wait_settlement()).all_released());
    assert!(cleanup_released.load(Ordering::SeqCst));
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn managed_write_rejections_return_request_and_release_fact_capacity() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeWriteProvider::new(FakeWriteMode::PrepareReject);
    let binding = FsExecutionBinding::new(manager.clone());
    let rejected = match binding.submit_write(
        &provider,
        write_request("/prepare/reject", b"owned"),
        &effects,
    ) {
        Ok(_) => panic!("prepare rejection expected"),
        Err(rejected) => rejected,
    };
    assert_eq!(rejected.request.path.as_str(), "/prepare/reject");
    assert_eq!(rejected.request.contents, b"owned");
    assert_eq!(
        rejected.preparation,
        Some(qubit_fs::FsPrepareFailureKind::Rejected("prepare".into()))
    );
    assert!(
        effects.reserve().is_ok(),
        "prepare rejection returns the slot"
    );

    let provider = FakeWriteProvider::new(FakeWriteMode::InstallReject);
    let rejected = match binding.submit_write(
        &provider,
        write_request("/install/reject", b"same"),
        &effects,
    ) {
        Ok(_) => panic!("reservation installation rejection expected"),
        Err(rejected) => rejected,
    };
    assert_eq!(rejected.request.path.as_str(), "/install/reject");
    assert_eq!(rejected.request.contents, b"same");
    assert_eq!(
        rejected.preparation,
        Some(qubit_fs::FsPrepareFailureKind::ReservationInstallFailed)
    );
    assert!(
        effects.reserve().is_ok(),
        "install rejection returns the slot"
    );
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

fn write_request(path: &str, contents: &[u8]) -> qubit_fs::FsOwnedWriteRequest {
    qubit_fs::FsOwnedWriteRequest {
        path: Path::parse(path).unwrap(),
        contents: contents.to_vec(),
        options: Default::default(),
    }
}

enum FakeWriteMode {
    PrepareReject,
    InstallReject,
    Success {
        gate: Option<(Arc<FakeActivationGate>, std::sync::mpsc::Sender<()>)>,
        cleanup_released: Arc<std::sync::atomic::AtomicBool>,
        settlement_gate: Arc<FakeSettlementGate>,
        resource_ids: Arc<std::sync::Mutex<Vec<qubit_operation::ResourceId>>>,
        stop_requests: Arc<std::sync::Mutex<Vec<qubit_operation::StopReason>>>,
        prepared_probe: Arc<std::sync::Mutex<Option<Arc<FakePreparedWrite>>>>,
    },
}

struct FakeWriteProvider(FakeWriteMode);
impl FakeWriteProvider {
    fn new(mode: FakeWriteMode) -> Self {
        Self(mode)
    }
}
impl qubit_fs::spi::ManagedFsMutationSpi for FakeWriteProvider {
    fn prepare_write(
        &self,
        request: qubit_fs::FsOwnedWriteRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsWrite>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedWriteRequest>,
    > {
        match &self.0 {
            FakeWriteMode::PrepareReject => Err(qubit_fs::FsPrepareFailure {
                kind: qubit_fs::FsPrepareFailureKind::Rejected("prepare".into()),
                request,
            }),
            FakeWriteMode::InstallReject => Ok(Arc::new(FakePreparedWrite::new(
                request,
                true,
                None,
                Arc::new(std::sync::atomic::AtomicBool::new(false)),
                Arc::new(FakeSettlementGate::default()),
                Arc::new(std::sync::Mutex::new(Vec::new())),
                Arc::new(std::sync::Mutex::new(Vec::new())),
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
        request: qubit_fs::FsOwnedCopyRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsCopy>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedCopyRequest>,
    > {
        Err(qubit_fs::FsPrepareFailure {
            kind: qubit_fs::FsPrepareFailureKind::Unsupported,
            request,
        })
    }
    fn prepare_rename(
        &self,
        request: qubit_fs::FsOwnedRenameRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsRename>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedRenameRequest>,
    > {
        Err(qubit_fs::FsPrepareFailure {
            kind: qubit_fs::FsPrepareFailureKind::Unsupported,
            request,
        })
    }
}

struct FakePreparedWrite {
    request: std::sync::Mutex<Option<qubit_fs::FsOwnedWriteRequest>>,
    id: std::sync::Mutex<Option<qubit_operation::OperationId>>,
    reservation: std::sync::Mutex<Option<qubit_fs::FsEffectReservation>>,
    outcome: std::sync::Mutex<
        Option<
            qubit_operation::OperationOutcome<
                qubit_fs::metadata::WriteOutcome,
                qubit_fs::FsOperationFailure,
            >,
        >,
    >,
    activated: std::sync::atomic::AtomicBool,
    install_reject: bool,
    gate: Option<(Arc<FakeActivationGate>, std::sync::mpsc::Sender<()>)>,
    cleanup_released: Arc<std::sync::atomic::AtomicBool>,
    settlement_gate: Arc<FakeSettlementGate>,
    resource_ids: Arc<std::sync::Mutex<Vec<qubit_operation::ResourceId>>>,
    stop_requests: Arc<std::sync::Mutex<Vec<qubit_operation::StopReason>>>,
    resource_ref: std::sync::Mutex<Option<qubit_operation::ResourceRef<()>>>,
    resource_reservation: std::sync::Mutex<Option<qubit_operation::ResourceReservation>>,
}
impl FakePreparedWrite {
    fn new(
        request: qubit_fs::FsOwnedWriteRequest,
        install_reject: bool,
        gate: Option<(Arc<FakeActivationGate>, std::sync::mpsc::Sender<()>)>,
        cleanup_released: Arc<std::sync::atomic::AtomicBool>,
        settlement_gate: Arc<FakeSettlementGate>,
        resource_ids: Arc<std::sync::Mutex<Vec<qubit_operation::ResourceId>>>,
        stop_requests: Arc<std::sync::Mutex<Vec<qubit_operation::StopReason>>>,
    ) -> Self {
        Self {
            request: std::sync::Mutex::new(Some(request)),
            id: std::sync::Mutex::new(None),
            reservation: std::sync::Mutex::new(None),
            outcome: std::sync::Mutex::new(None),
            activated: std::sync::atomic::AtomicBool::new(false),
            install_reject,
            gate,
            cleanup_released,
            settlement_gate,
            resource_ids,
            stop_requests,
            resource_ref: std::sync::Mutex::new(None),
            resource_reservation: std::sync::Mutex::new(None),
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
        if let Some(resource_reservation) = self.resource_reservation.lock().unwrap().take() {
            if let Ok(resource) = resource_reservation.commit_with((), FakeWriteCleanup) {
                self.resource_ids.lock().unwrap().push(resource.id());
                *self.resource_ref.lock().unwrap() = Some(resource);
            }
        }
        let fact = if self.stop_requests.lock().unwrap().is_empty() {
            qubit_fs::FsEffectState::Applied
        } else {
            qubit_fs::FsEffectState::Indeterminate
        };
        reservation.publish(qubit_fs::FsEffectFact::State(fact));
        let bytes = self
            .request
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .contents
            .len() as u64;
        *self.outcome.lock().unwrap() = Some(qubit_operation::OperationOutcome::Succeeded(
            qubit_fs::metadata::WriteOutcome::new(
                qubit_fs::metadata::AchievedAtomicity::Atomic,
                qubit_fs::metadata::PublicationMethod::Direct,
            )
            .with_bytes_written(bytes),
        ));
        self.activated.store(true, Ordering::SeqCst);
        drop(reservation);
    }
    fn final_fact(&self) -> bool {
        self.activated.load(Ordering::SeqCst)
    }
}
impl qubit_operation::OperationControl for FakePreparedWrite {
    fn request_stop(&self, reason: qubit_operation::StopReason) {
        self.stop_requests.lock().unwrap().push(reason);
    }
    fn snapshot(&self) -> qubit_operation::ChildSnapshot {
        let operation_id = self.id.lock().unwrap().unwrap();
        let done = self.final_fact();
        qubit_operation::ChildSnapshot {
            operation_id,
            execution: if done {
                qubit_operation::ExecutionPhase::Finished
            } else {
                qubit_operation::ExecutionPhase::Running
            },
            stop: None,
            outcome: done.then_some(qubit_operation::ChildOutcomeSummary {
                operation_id,
                kind: qubit_operation::ChildOutcomeKind::Succeeded,
                summary: qubit_operation::DiagnosticSummary::new("child succeeded"),
            }),
            input_released: done,
            settlement: done.then(|| Arc::new(fake_settlement(operation_id))),
        }
    }
    fn poll_outcome(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<qubit_operation::ChildOutcomeSummary> {
        if self.final_fact() {
            std::task::Poll::Ready(self.snapshot().outcome.unwrap())
        } else {
            std::task::Poll::Pending
        }
    }
    fn poll_release(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<qubit_operation::ChildRelease> {
        if self.final_fact() {
            std::task::Poll::Ready(qubit_operation::ChildRelease {
                operation_id: self.id.lock().unwrap().unwrap(),
                confirmed_at: None,
            })
        } else {
            std::task::Poll::Pending
        }
    }
    fn poll_settlement(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Arc<qubit_operation::SettlementReport>> {
        if self.final_fact() && self.settlement_gate.open.load(Ordering::SeqCst) {
            self.resource_ref.lock().unwrap().take();
            self.cleanup_released.store(true, Ordering::SeqCst);
            std::task::Poll::Ready(Arc::new(fake_settlement(self.id.lock().unwrap().unwrap())))
        } else {
            *self.settlement_gate.waker.lock().unwrap() = Some(cx.waker().clone());
            std::task::Poll::Pending
        }
    }
}
impl
    qubit_operation::PreparedExternalOperation<
        qubit_fs::metadata::WriteOutcome,
        qubit_fs::FsOperationFailure,
    > for FakePreparedWrite
{
    fn bind(
        &self,
        id: qubit_operation::OperationId,
    ) -> Result<(), qubit_operation::ExternalBindError> {
        let mut bound = self.id.lock().unwrap();
        if bound.is_some() {
            return Err(qubit_operation::ExternalBindError::AlreadyBound);
        }
        *bound = Some(id);
        Ok(())
    }
    fn activate(
        self: Arc<Self>,
        scope: qubit_operation::OperationScope,
    ) -> Result<(), qubit_operation::ExecutionFailure> {
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
            reservation.publish(qubit_fs::FsEffectFact::State(
                qubit_fs::FsEffectState::Unchanged,
            ));
        }
    }
    fn poll_take_outcome(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        qubit_operation::OperationOutcome<
            qubit_fs::metadata::WriteOutcome,
            qubit_fs::FsOperationFailure,
        >,
    > {
        if let Some((gate, _)) = &self.gate {
            if !gate.poll_open(cx) {
                return std::task::Poll::Pending;
            }
            self.complete_success();
        }
        self.outcome
            .lock()
            .unwrap()
            .take()
            .map_or(std::task::Poll::Pending, std::task::Poll::Ready)
    }
}

struct FakeWriteCleanup;
impl qubit_operation::AsyncCleanup<()> for FakeWriteCleanup {
    fn cleanup(
        self: Arc<Self>,
        _: (),
        _: qubit_operation::CleanupContext,
    ) -> qubit_operation::CleanupFuture<()> {
        Box::pin(async { qubit_operation::CleanupOutcome::Released })
    }
}
impl qubit_fs::spi::PreparedFsWrite for FakePreparedWrite {
    fn take_unclaimed_request(&self) -> Option<qubit_fs::FsOwnedWriteRequest> {
        if self.id.lock().unwrap().is_some() {
            return None;
        }
        self.reservation.lock().unwrap().take();
        self.request.lock().unwrap().take()
    }
    fn install_effect_reservation(
        &self,
        reservation: qubit_fs::FsEffectReservation,
    ) -> Result<(), qubit_fs::FsEffectReservation> {
        if self.install_reject || self.reservation.lock().unwrap().is_some() {
            Err(reservation)
        } else {
            *self.reservation.lock().unwrap() = Some(reservation);
            Ok(())
        }
    }
}

fn fake_settlement(
    operation_id: qubit_operation::OperationId,
) -> qubit_operation::SettlementReport {
    qubit_operation::SettlementReport {
        operation_id,
        status: qubit_operation::SettlementStatus::Released,
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
    }
}

#[derive(Default)]
struct FakeActivationGate {
    open: std::sync::atomic::AtomicBool,
    waker: std::sync::Mutex<Option<std::task::Waker>>,
}
impl FakeActivationGate {
    fn poll_open(&self, cx: &std::task::Context<'_>) -> bool {
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
    open: std::sync::atomic::AtomicBool,
    waker: std::sync::Mutex<Option<std::task::Waker>>,
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
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct ThreadWaker(std::thread::Thread);
    impl std::task::Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(output) => return output,
            std::task::Poll::Pending => std::thread::park(),
        }
    }
}

#[test]
fn manager_admission_rejection_recovers_write_request_and_reserved_slot() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released,
        settlement_gate: Arc::new(FakeSettlementGate::default()),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager);
    let rejected = match binding.submit_write(
        &provider,
        write_request("/admission/reject", b"owned input"),
        &effects,
    ) {
        Ok(_) => panic!("closed manager must reject write"),
        Err(rejected) => rejected,
    };
    assert!(rejected.admission.is_some());
    assert!(rejected.preparation.is_none());
    assert_eq!(rejected.request.path.as_str(), "/admission/reject");
    assert_eq!(rejected.request.contents, b"owned input");
    assert!(
        effects.reserve().is_ok(),
        "unclaimed prepared owner released its slot"
    );
}

#[test]
fn managed_tree_publish_uses_one_fact_slot_and_preserves_typed_publication_facts() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeTreeProvider;
    let binding = FsExecutionBinding::new(manager.clone());
    let submitted = binding
        .submit_tree_publish(&provider, tree_request(), &effects)
        .unwrap_or_else(|_| panic!("tree publication should be prepared"));
    let mut operation = submitted.operation;
    assert!(matches!(
        operation.wait_outcome().unwrap(),
        qubit_operation::OperationOutcome::Succeeded(outcome) if outcome.entries_published == 2 && outcome.content_bytes == 11
    ));
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::TreePublish { outcome, recovery: None })
            if outcome.entries_published == 2 && outcome.content_bytes == 11 && outcome.durable
    ));
    drop(submitted.effect);
    assert!(operation.wait_settlement().unwrap().all_released());
    assert!(effects.reserve().is_ok());
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

fn tree_request() -> qubit_fs::FsOwnedTreePublishRequest {
    qubit_fs::FsOwnedTreePublishRequest {
        staging_root: Path::parse("/stage").unwrap(),
        destination_root: Path::parse("/destination").unwrap(),
        validation: Default::default(),
        validated_version: None,
        publication: Default::default(),
    }
}

struct FakeTreeProvider;
impl qubit_fs::spi::ManagedFsTreeSpi for FakeTreeProvider {
    fn prepare_tree_publish(
        &self,
        request: qubit_fs::FsOwnedTreePublishRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsTreePublish>,
        qubit_fs::FsPrepareFailure<qubit_fs::FsOwnedTreePublishRequest>,
    > {
        Ok(Arc::new(FakePreparedTree {
            request: std::sync::Mutex::new(Some(request)),
            id: std::sync::Mutex::new(None),
            reservation: std::sync::Mutex::new(None),
            outcome: std::sync::Mutex::new(None),
            completed: std::sync::atomic::AtomicBool::new(false),
        }))
    }
}

struct FakePreparedTree {
    request: std::sync::Mutex<Option<qubit_fs::FsOwnedTreePublishRequest>>,
    id: std::sync::Mutex<Option<qubit_operation::OperationId>>,
    reservation: std::sync::Mutex<Option<qubit_fs::FsEffectReservation>>,
    outcome: std::sync::Mutex<
        Option<
            qubit_operation::OperationOutcome<
                qubit_fs::directory::TreePublishOutcome,
                qubit_fs::FsOperationFailure,
            >,
        >,
    >,
    completed: std::sync::atomic::AtomicBool,
}
impl qubit_operation::OperationControl for FakePreparedTree {
    fn request_stop(&self, _: qubit_operation::StopReason) {}
    fn snapshot(&self) -> qubit_operation::ChildSnapshot {
        let operation_id = self.id.lock().unwrap().unwrap();
        let done = self.completed.load(Ordering::SeqCst);
        qubit_operation::ChildSnapshot {
            operation_id,
            execution: if done {
                qubit_operation::ExecutionPhase::Finished
            } else {
                qubit_operation::ExecutionPhase::Running
            },
            stop: None,
            outcome: done.then_some(qubit_operation::ChildOutcomeSummary {
                operation_id,
                kind: qubit_operation::ChildOutcomeKind::Succeeded,
                summary: qubit_operation::DiagnosticSummary::new("child succeeded"),
            }),
            input_released: done,
            settlement: done.then(|| Arc::new(fake_settlement(operation_id))),
        }
    }
    fn poll_outcome(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<qubit_operation::ChildOutcomeSummary> {
        if self.completed.load(Ordering::SeqCst) {
            std::task::Poll::Ready(self.snapshot().outcome.unwrap())
        } else {
            std::task::Poll::Pending
        }
    }
    fn poll_release(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<qubit_operation::ChildRelease> {
        if self.completed.load(Ordering::SeqCst) {
            std::task::Poll::Ready(qubit_operation::ChildRelease {
                operation_id: self.id.lock().unwrap().unwrap(),
                confirmed_at: None,
            })
        } else {
            std::task::Poll::Pending
        }
    }
    fn poll_settlement(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Arc<qubit_operation::SettlementReport>> {
        if self.completed.load(Ordering::SeqCst) {
            std::task::Poll::Ready(Arc::new(fake_settlement(self.id.lock().unwrap().unwrap())))
        } else {
            std::task::Poll::Pending
        }
    }
}
impl
    qubit_operation::PreparedExternalOperation<
        qubit_fs::directory::TreePublishOutcome,
        qubit_fs::FsOperationFailure,
    > for FakePreparedTree
{
    fn bind(
        &self,
        id: qubit_operation::OperationId,
    ) -> Result<(), qubit_operation::ExternalBindError> {
        let mut bound = self.id.lock().unwrap();
        if bound.is_some() {
            return Err(qubit_operation::ExternalBindError::AlreadyBound);
        }
        *bound = Some(id);
        Ok(())
    }
    fn activate(
        self: Arc<Self>,
        _: qubit_operation::OperationScope,
    ) -> Result<(), qubit_operation::ExecutionFailure> {
        let outcome = qubit_fs::directory::TreePublishOutcome {
            entries_published: 2,
            content_bytes: 11,
            atomicity: qubit_fs::metadata::AchievedAtomicity::Atomic,
            method: qubit_fs::metadata::PublicationMethod::AtomicRename,
            durable: true,
        };
        let reservation = self
            .reservation
            .lock()
            .unwrap()
            .take()
            .expect("binding installed effect reservation");
        reservation.publish_tree_outcome(outcome.clone(), None);
        *self.outcome.lock().unwrap() = Some(qubit_operation::OperationOutcome::Succeeded(outcome));
        self.completed.store(true, Ordering::SeqCst);
        drop(reservation);
        Ok(())
    }
    fn abandon(self: Arc<Self>) {
        if let Some(reservation) = self.reservation.lock().unwrap().take() {
            reservation.publish(qubit_fs::FsEffectFact::State(
                qubit_fs::FsEffectState::Unchanged,
            ));
        }
    }
    fn poll_take_outcome(
        &self,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<
        qubit_operation::OperationOutcome<
            qubit_fs::directory::TreePublishOutcome,
            qubit_fs::FsOperationFailure,
        >,
    > {
        self.outcome
            .lock()
            .unwrap()
            .take()
            .map_or(std::task::Poll::Pending, std::task::Poll::Ready)
    }
}
impl qubit_fs::spi::PreparedFsTreePublish for FakePreparedTree {
    fn take_unclaimed_request(&self) -> Option<qubit_fs::FsOwnedTreePublishRequest> {
        if self.id.lock().unwrap().is_some() {
            return None;
        }
        self.reservation.lock().unwrap().take();
        self.request.lock().unwrap().take()
    }
    fn install_effect_reservation(
        &self,
        reservation: qubit_fs::FsEffectReservation,
    ) -> Result<(), qubit_fs::FsEffectReservation> {
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
fn stop_control_remains_available_while_sync_executor_queue_is_saturated() {
    use std::time::Duration;

    let options = qubit_operation::ManagerOptions {
        limits: qubit_operation::ManagerLimits {
            active_operations: 4,
            queued_operations: 8,
            admission_waiters: 8,
            ..Default::default()
        },
        sync_executor: qubit_operation::ThreadPoolExecutorOptions {
            worker_count: 1,
            queue_capacity: 1,
        },
        ..Default::default()
    };
    let manager = qubit_operation::OperationManager::new(options).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let cleanup_released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let stop_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released,
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
    });
    let binding = FsExecutionBinding::new(manager.clone());
    let mut active = binding
        .submit_write(
            &provider,
            write_request("/queue/active", b"active"),
            &effects,
        )
        .unwrap_or_else(|_| panic!("active managed write should be admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("the only executor worker entered provider activation");

    let mut queued = manager
        .try_submit(Default::default(), |_| Ok::<_, ()>(()))
        .unwrap_or_else(|_| panic!("single queue slot should accept one operation"));
    let saturated = manager.try_submit(Default::default(), |_| Ok::<_, ()>(()));
    assert!(saturated.is_err(), "executor queue is full");
    assert_eq!(
        active
            .operation
            .request_stop(qubit_operation::StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted,
        "stop request is delivered through the handle without using the saturated work queue"
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        *stop_requests.lock().unwrap(),
        vec![qubit_operation::StopReason::UserCancelled],
        "the prepared provider receives the stop exactly once"
    );

    gate.release();
    assert!(matches!(
        active.operation.wait_outcome().unwrap(),
        qubit_operation::OperationOutcome::Stopped(_)
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
        qubit_operation::OperationOutcome::Succeeded(())
    ));
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn dropped_handle_keeps_pending_remote_owner_until_input_release_is_confirmed() {
    use qubit_operation::OperationControl;
    use std::time::Duration;

    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let stop_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let prepared_probe = Arc::new(std::sync::Mutex::new(None));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::clone(&prepared_probe),
    });
    let submitted = FsExecutionBinding::new(manager.clone())
        .submit_write(
            &provider,
            write_request("/remote/pending", b"owned"),
            &effects,
        )
        .unwrap_or_else(|_| panic!("managed remote operation should be admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("provider activation returned while remote work is pending");
    let prepared = prepared_probe.lock().unwrap().clone().unwrap();
    assert!(!prepared.snapshot().input_released);
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert!(matches!(
        prepared.poll_release(&mut context),
        std::task::Poll::Pending
    ));
    assert!(matches!(
        prepared.poll_settlement(&mut context),
        std::task::Poll::Pending
    ));
    assert!(matches!(
        submitted.effect.snapshot(),
        FsEffectSnapshot::Pending
    ));
    assert!(
        effects.reserve().is_err(),
        "the running provider retains its owner and slot"
    );

    let observer = submitted.effect;
    drop(submitted.operation);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        *stop_requests.lock().unwrap(),
        vec![qubit_operation::StopReason::UserCancelled]
    );
    assert!(
        !prepared.snapshot().input_released,
        "remote work still consumes input"
    );
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(effects.reserve().is_err());

    gate.release();
    assert!(matches!(
        observer.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !prepared.snapshot().input_released && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(
        prepared.snapshot().input_released,
        "provider confirms it released the owned input"
    );
    assert!(matches!(
        prepared.poll_release(&mut context),
        std::task::Poll::Ready(_)
    ));
    settlement_gate.release();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut reclaimed = false;
    while std::time::Instant::now() < deadline {
        if let Ok(reservation) = effects.reserve() {
            drop(reservation);
            reclaimed = true;
            break;
        }
        std::thread::yield_now();
    }
    assert!(
        reclaimed,
        "the fact slot returns after release and recovery responsibility ends"
    );
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn execution_binding_group_keeps_parent_target_for_children_after_group_close() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let group = manager
        .group(qubit_operation::GroupOptions { capacity: 4 })
        .unwrap();
    let effects = FsEffectSlotPool::new(1);
    let cleanup_released = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released,
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
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
                    .submit_write(
                        &provider,
                        write_request("/group/child", b"group-owned"),
                        &effects,
                    )
                    .unwrap_or_else(|_| panic!("an already-bound parent group admits its child"));
                let mut operation = submitted.operation;
                assert!(matches!(
                    operation.wait_outcome().unwrap(),
                    qubit_operation::OperationOutcome::Succeeded(value) if value.bytes_written() == Some(11)
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
        qubit_operation::OperationOutcome::Succeeded(())
    ));
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn execution_binding_rejects_wrong_manager_closed_and_over_limit_groups() {
    let manager = qubit_operation::OperationManager::new(qubit_operation::ManagerOptions {
        max_groups_per_operation: 1,
        ..Default::default()
    })
    .unwrap();
    let other = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let group_a = manager
        .group(qubit_operation::GroupOptions { capacity: 4 })
        .unwrap();
    let group_b = manager
        .group(qubit_operation::GroupOptions { capacity: 4 })
        .unwrap();
    let foreign_group = other
        .group(qubit_operation::GroupOptions { capacity: 4 })
        .unwrap();
    let binding = FsExecutionBinding::new(manager.clone());
    assert!(matches!(
        binding.with_group(&foreign_group),
        Err(qubit_operation::GroupBindingError::WrongManager)
    ));
    let bound = binding.with_group(&group_a).unwrap();
    assert!(matches!(
        bound.with_group(&group_b),
        Err(qubit_operation::GroupBindingError::CapacityExceeded)
    ));
    group_a.close_admission();
    assert!(matches!(
        binding.with_group(&group_a),
        Err(qubit_operation::GroupBindingError::Closed)
    ));
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
    let _ = other
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[cfg(feature = "async")]
#[test]
fn async_binding_keeps_service_group_admission_constraints() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let group = manager
        .group(qubit_operation::GroupOptions { capacity: 2 })
        .unwrap();
    let binding = FsExecutionBinding::new(manager.clone())
        .with_group(&group)
        .unwrap();
    group.close_admission();
    let effects = FsEffectSlotPool::new(1);
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: None,
        cleanup_released: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        settlement_gate: Arc::new(FakeSettlementGate::default()),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::new(std::sync::Mutex::new(Vec::new())),
        prepared_probe: Arc::new(std::sync::Mutex::new(None)),
    });
    let error = match binding.submit_write_async(
        &provider,
        write_request("/async/group-closed", b"returned"),
        &effects,
    ) {
        Ok(_) => panic!("async manager view must retain the closed group boundary"),
        Err(error) => error,
    };
    assert!(error.admission.is_some());
    assert_eq!(error.request.path.as_str(), "/async/group-closed");
    assert_eq!(error.request.contents, b"returned");
    assert!(effects.reserve().is_ok());
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[cfg(feature = "async")]
#[test]
fn managed_async_tree_publish_preserves_typed_effect_facts() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let submitted = FsExecutionBinding::new(manager.clone())
        .submit_tree_publish_async(&FakeTreeProvider, tree_request(), &effects)
        .unwrap_or_else(|_| panic!("async tree publication should be admitted"));
    let mut operation = submitted.operation;
    let outcome = block_on(operation.wait_outcome()).unwrap();
    assert!(matches!(
        outcome,
        qubit_operation::OperationOutcome::Succeeded(ref value)
            if value.entries_published == 2 && value.content_bytes == 11
    ));
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::TreePublish { outcome, recovery: None })
            if outcome.entries_published == 2 && outcome.durable
    ));
    drop(submitted.effect);
    assert!(block_on(operation.wait_settlement()).all_released());
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn unknown_remote_effect_with_released_input_can_settle_locally() {
    use qubit_operation::OperationControl;
    use std::time::Duration;

    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let effects = FsEffectSlotPool::new(1);
    let gate = Arc::new(FakeActivationGate::default());
    let settlement_gate = Arc::new(FakeSettlementGate::default());
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let stop_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let prepared_probe = Arc::new(std::sync::Mutex::new(None));
    let provider = FakeWriteProvider::new(FakeWriteMode::Success {
        gate: Some((Arc::clone(&gate), entered_tx)),
        cleanup_released: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        settlement_gate: Arc::clone(&settlement_gate),
        resource_ids: Arc::new(std::sync::Mutex::new(Vec::new())),
        stop_requests: Arc::clone(&stop_requests),
        prepared_probe: Arc::clone(&prepared_probe),
    });
    let mut submitted = FsExecutionBinding::new(manager.clone())
        .submit_write(
            &provider,
            write_request("/remote/released", b"input"),
            &effects,
        )
        .unwrap_or_else(|_| panic!("remote operation admitted"));
    entered_rx
        .recv_timeout(Duration::from_secs(3))
        .expect("prepared provider entered pending remote work");
    let prepared = prepared_probe.lock().unwrap().clone().unwrap();
    assert!(!prepared.snapshot().input_released);
    assert_eq!(
        submitted
            .operation
            .request_stop(qubit_operation::StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted
    );
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while stop_requests.lock().unwrap().is_empty() && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(
        *stop_requests.lock().unwrap(),
        vec![qubit_operation::StopReason::UserCancelled]
    );
    gate.release();
    assert!(matches!(
        submitted.effect.wait_final(),
        Ok(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !prepared.snapshot().input_released && std::time::Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(prepared.snapshot().input_released);
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    assert!(matches!(
        prepared.poll_release(&mut context),
        std::task::Poll::Ready(_)
    ));
    settlement_gate.release();
    assert!(matches!(
        submitted.operation.wait_outcome().unwrap(),
        qubit_operation::OperationOutcome::Stopped(_)
    ));
    assert!(
        submitted
            .operation
            .wait_settlement()
            .unwrap()
            .all_released()
    );
    assert!(
        effects.reserve().is_ok(),
        "released local input permits settlement with unknown remote effect"
    );
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}
