use std::sync::Arc;
use std::sync::Barrier;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use qubit_fs::FsOperationFailure;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::execution::PreparedFsCopyAdapter;
use qubit_fs::execution::PreparedFsReadAdapter;
use qubit_fs::execution::PreparedFsRenameAdapter;
use qubit_fs::execution::PreparedFsTreePublishAdapter;
use qubit_fs::execution::PreparedFsWriteAdapter;
use qubit_fs::spi::PreparedFsCopy;
use qubit_fs::spi::PreparedFsRead;
use qubit_fs::spi::PreparedFsRename;
use qubit_fs::spi::PreparedFsTreePublish;
use qubit_fs::spi::PreparedFsWrite;
use qubit_operation::ChildOutcomeSummary;
use qubit_operation::ChildRelease;
use qubit_operation::ChildSnapshot;
use qubit_operation::ExecutionFailure;
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
use qubit_operation::StopReason;

struct BindGate {
    entered: Barrier,
    release: Barrier,
}

struct PreparedRead {
    gate: Option<Arc<BindGate>>,
    reject_bind: bool,
    binds: AtomicUsize,
    request: Mutex<Option<FsOwnedReadRequest>>,
}

impl OperationControl for PreparedRead {
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
    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        Poll::Pending
    }
}

impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for PreparedRead {
    fn bind(&self, _: OperationId) -> Result<(), ExternalBindError> {
        self.binds.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.entered.wait();
            gate.release.wait();
        }
        if self.reject_bind {
            Err(ExternalBindError::AlreadyBound)
        } else {
            Ok(())
        }
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

impl PreparedFsRead for PreparedRead {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.request.lock().unwrap().take()
    }
}

#[test]
fn request_recovery_does_not_wait_for_provider_bind() {
    let gate = Arc::new(BindGate {
        entered: Barrier::new(2),
        release: Barrier::new(2),
    });
    let prepared = Arc::new(PreparedRead {
        gate: Some(Arc::clone(&gate)),
        reject_bind: false,
        binds: AtomicUsize::new(0),
        request: Mutex::new(Some(FsOwnedReadRequest {
            path: Path::parse("/bind-window").unwrap(),
            options: ReadAllOptions::default(),
        })),
    });
    let adapter = Arc::new(PreparedFsReadAdapter::new(prepared));
    let binding_adapter = Arc::clone(&adapter);
    let binding = std::thread::spawn(move || {
        PreparedExternalOperation::bind(binding_adapter.as_ref(), fresh_operation_id())
    });
    gate.entered.wait();

    let request_adapter = Arc::clone(&adapter);
    let (sender, receiver) = mpsc::channel();
    let recovery = std::thread::spawn(move || sender.send(request_adapter.take_request()).unwrap());
    let recovered_during_bind = receiver.recv_timeout(Duration::from_millis(100));
    gate.release.wait();

    binding.join().unwrap().unwrap();
    recovery.join().unwrap();
    assert!(
        recovered_during_bind.is_ok(),
        "request recovery blocked inside provider bind"
    );
    assert!(
        recovered_during_bind.unwrap().is_none(),
        "request recovered after binding began"
    );
}

fn adapter(reject_bind: bool) -> (Arc<PreparedFsReadAdapter>, Arc<PreparedRead>) {
    let prepared = Arc::new(PreparedRead {
        gate: None,
        reject_bind,
        binds: AtomicUsize::new(0),
        request: Mutex::new(Some(FsOwnedReadRequest {
            path: Path::parse("/claim-contract").unwrap(),
            options: ReadAllOptions::default(),
        })),
    });
    let trait_object: Arc<dyn PreparedFsRead> = Arc::clone(&prepared) as Arc<dyn PreparedFsRead>;
    (Arc::new(PreparedFsReadAdapter::new(trait_object)), prepared)
}

#[test]
fn rejected_bind_rolls_back_and_keeps_request_recoverable() {
    let (adapter, prepared) = adapter(true);
    assert!(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).is_err());
    assert_eq!(
        adapter.take_request().unwrap().path.as_str(),
        "/claim-contract"
    );
    assert!(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).is_err());
    assert_eq!(prepared.binds.load(Ordering::SeqCst), 1);
}

#[test]
fn successful_bind_prevents_recovery_and_repeated_provider_claim() {
    let (adapter, prepared) = adapter(false);
    PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).unwrap();
    assert!(adapter.take_request().is_none());
    assert!(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).is_err());
    assert_eq!(prepared.binds.load(Ordering::SeqCst), 1);
}

macro_rules! mutation_claim_contract {
    ($prepared:ident, $spi:path, $adapter:ty, $output:ty, $request:ty, $test:ident) => {
        struct $prepared {
            binds: AtomicUsize,
            request_reads: AtomicUsize,
        }

        impl OperationControl for $prepared {
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
            fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
                Poll::Pending
            }
        }

        impl PreparedExternalOperation<$output, FsOperationFailure> for $prepared {
            fn bind(&self, _: OperationId) -> Result<(), ExternalBindError> {
                self.binds.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
                Ok(())
            }
            fn abandon(self: Arc<Self>) {}
            fn poll_take_outcome(
                &self,
                _: &mut Context<'_>,
            ) -> Poll<OperationOutcome<$output, FsOperationFailure>> {
                Poll::Pending
            }
        }

        impl $spi for $prepared {
            fn take_unclaimed_request(&self) -> Option<$request> {
                self.request_reads.fetch_add(1, Ordering::SeqCst);
                None
            }
            fn install_effect_reservation(
                &self,
                reservation: qubit_fs::execution::FsEffectReservation,
            ) -> Result<(), qubit_fs::execution::FsEffectReservation> {
                Err(reservation)
            }
        }

        #[test]
        fn $test() {
            let prepared = Arc::new($prepared {
                binds: AtomicUsize::new(0),
                request_reads: AtomicUsize::new(0),
            });
            let task: Arc<dyn $spi> = Arc::clone(&prepared) as Arc<dyn $spi>;
            let adapter = <$adapter>::new(task);
            PreparedExternalOperation::bind(&adapter, fresh_operation_id()).unwrap();
            assert!(adapter.take_request().is_none());
            assert!(PreparedExternalOperation::bind(&adapter, fresh_operation_id()).is_err());
            assert_eq!(prepared.binds.load(Ordering::SeqCst), 1);
            assert_eq!(prepared.request_reads.load(Ordering::SeqCst), 0);
        }
    };
}

mutation_claim_contract!(
    PreparedCopyFixture,
    PreparedFsCopy,
    PreparedFsCopyAdapter,
    qubit_fs::copy::CopyOutcome,
    qubit_fs::execution::FsOwnedCopyRequest,
    copy_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedRenameFixture,
    PreparedFsRename,
    PreparedFsRenameAdapter,
    qubit_fs::rename::RenameOutcome,
    qubit_fs::execution::FsOwnedRenameRequest,
    rename_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedTreePublishFixture,
    PreparedFsTreePublish,
    PreparedFsTreePublishAdapter,
    qubit_fs::directory::TreePublishOutcome,
    qubit_fs::execution::FsOwnedTreePublishRequest,
    tree_publish_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedWriteFixture,
    PreparedFsWrite,
    PreparedFsWriteAdapter,
    qubit_fs::metadata::WriteOutcome,
    qubit_fs::execution::FsOwnedWriteRequest,
    write_claim_blocks_recovery_and_repeated_bind
);

fn fresh_operation_id() -> OperationId {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let mut operation = manager
        .try_submit(OperationOptions::default(), |_| Ok::<(), ()>(()))
        .unwrap();
    let id = operation.id();
    let _ = operation.wait_outcome().unwrap();
    id
}
