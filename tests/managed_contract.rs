use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use qubit_fs::spi::{ManagedFsReadSpi, PreparedFsRead};
use qubit_fs::{
    AsyncFsOperation, FsEffectSlotPool, FsEffectSnapshot, FsEffectState, FsOperation,
    FsOperationFailure, FsPrepareFailure, FsSubmitFailure, Path, ReadAllOptions,
};
use qubit_operation::{
    ChildOutcomeSummary, ChildRelease, ChildSnapshot, ExecutionFailure, OperationControl,
    OperationId, OperationManager, OperationOutcome, OperationScope, PreparedExternalOperation,
    ShutdownMode, StopReason,
};

#[test]
fn operation_aliases_are_public() {
    let _: Option<FsOperation<Vec<u8>>> = None;
    let _: Option<AsyncFsOperation<Vec<u8>>> = None;
}

#[test]
fn common_owned_contract_is_exported_and_object_safe() {
    fn accepts_provider(_: &dyn ManagedFsReadSpi) {}
    let _ = accepts_provider;
    let _: Option<FsOperationFailure> = None;
    let _: Option<FsPrepareFailure<qubit_fs::FsOwnedReadRequest>> = None;
    let _: Option<FsSubmitFailure<qubit_fs::FsOwnedReadRequest>> = None;
    let _: Option<Arc<dyn PreparedFsRead>> = None;
    let request = qubit_fs::FsOwnedReadRequest {
        path: Path::parse("/data").unwrap(),
        options: ReadAllOptions::default(),
    };
    assert_eq!(request.path.as_str(), "/data");
}

#[test]
fn preparation_failure_returns_the_original_request() {
    let request = qubit_fs::FsOwnedReadRequest {
        path: Path::parse("/same-request").unwrap(),
        options: ReadAllOptions::new(7, qubit_fs::ReadConsistencyPolicy::ObserveChanges),
    };
    let failure = FsPrepareFailure {
        kind: qubit_fs::execution::FsPrepareFailureKind::Unsupported,
        request,
    };
    let (kind, recovered) = failure.into_parts();
    assert_eq!(kind, qubit_fs::execution::FsPrepareFailureKind::Unsupported);
    assert_eq!(recovered.path.as_str(), "/same-request");
    assert_eq!(recovered.options.max_bytes, 7);
}

#[test]
fn managed_submission_rejection_returns_the_owned_request() {
    let manager = OperationManager::new(qubit_operation::ManagerOptions::default()).unwrap();
    let shutdown = manager.request_shutdown(ShutdownMode::Drain);
    let _report = shutdown.wait().unwrap();
    let binding = qubit_fs::FsExecutionBinding::new(manager);
    let request = qubit_fs::FsOwnedReadRequest {
        path: Path::parse("/admission-reject").unwrap(),
        options: ReadAllOptions::default(),
    };
    let failure = match binding.submit_read_all(&HoldingProvider, request) {
        Ok(_) => panic!("closed manager unexpectedly accepted submission"),
        Err(failure) => failure,
    };
    let (reason, preparation, recovered) = failure.into_parts();
    assert!(
        reason.is_some(),
        "expected manager admission error: {reason:?}"
    );
    assert!(preparation.is_none());
    assert_eq!(recovered.path.as_str(), "/admission-reject");
}

#[test]
fn effect_fact_slots_are_bounded_reclaimed_and_final_is_immutable() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let clone = reservation.observer();
    let observer = clone.clone();
    assert!(pool.reserve().is_err());
    reservation.publish(qubit_fs::FsEffectFact::State(FsEffectState::Applied));
    clone.publish(FsEffectState::Indeterminate);
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(qubit_fs::FsEffectFact::State(FsEffectState::Applied))
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

struct HoldingProvider;
impl ManagedFsReadSpi for HoldingProvider {
    fn prepare_read_all(
        &self,
        request: qubit_fs::FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<qubit_fs::FsOwnedReadRequest>> {
        Ok(Arc::new(HoldingRead {
            request: Mutex::new(Some(request)),
        }))
    }
}

struct HoldingRead {
    request: Mutex<Option<qubit_fs::FsOwnedReadRequest>>,
}
impl OperationControl for HoldingRead {
    fn request_stop(&self, _: StopReason) {}
    fn snapshot(&self) -> ChildSnapshot {
        unimplemented!()
    }
    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        Poll::Pending
    }
    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        Poll::Pending
    }
    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<qubit_operation::SettlementReport>> {
        Poll::Pending
    }
}
impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for HoldingRead {
    fn bind(&self, _: OperationId) -> Result<(), qubit_operation::ExternalBindError> {
        Ok(())
    }
    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        Ok(())
    }
    fn abandon(self: Arc<Self>) {}
    fn poll_take_outcome(
        &self,
        _: &mut Context<'_>,
    ) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        Poll::Pending
    }
}
impl PreparedFsRead for HoldingRead {
    fn take_unclaimed_request(&self) -> Option<qubit_fs::FsOwnedReadRequest> {
        self.request
            .lock()
            .expect("request mutex not poisoned")
            .take()
    }
}

#[test]
fn effect_observer_waits_for_final_facts_in_sync_and_async_paths() {
    use std::future::Future;

    let pool = FsEffectSlotPool::new(2);
    let sync_reservation = pool.reserve().unwrap();
    let sync_observer = sync_reservation.observer();
    sync_reservation.publish(qubit_fs::FsEffectFact::State(FsEffectState::Unchanged));
    assert!(matches!(
        sync_observer.wait_final(),
        Ok(qubit_fs::FsEffectFact::State(FsEffectState::Unchanged))
    ));

    let async_reservation = pool.reserve().unwrap();
    let async_observer = async_reservation.observer();
    let mut wait = Box::pin(async_observer.wait_final_async());
    let waker = std::task::Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(wait.as_mut().poll(&mut context), Poll::Pending));
    async_reservation.publish(qubit_fs::FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        wait.as_mut().poll(&mut context),
        Poll::Ready(Ok(qubit_fs::FsEffectFact::State(FsEffectState::Applied)))
    ));
}
