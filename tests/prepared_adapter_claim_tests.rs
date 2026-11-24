// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;

use qubit_fs::FsOperationFailure;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::directory::TreePublishOutcome;
use qubit_fs::execution::FsEffectReservation;
use qubit_fs::execution::FsOwnedCopyRequest;
use qubit_fs::execution::FsOwnedRenameRequest;
use qubit_fs::execution::FsOwnedTreePublishRequest;
use qubit_fs::execution::FsOwnedWriteRequest;
use qubit_fs::execution::PreparedFsCopyAdapter;
use qubit_fs::execution::PreparedFsReadAdapter;
use qubit_fs::execution::PreparedFsRenameAdapter;
use qubit_fs::execution::PreparedFsTreePublishAdapter;
use qubit_fs::execution::PreparedFsWriteAdapter;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::rename::RenameOutcome;
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

struct CallbackGate {
    entered: SyncSender<&'static str>,
    release: Mutex<Receiver<()>>,
}

impl CallbackGate {
    fn pause(&self, stage: &'static str) -> bool {
        if self.entered.send(stage).is_err() {
            return false;
        }
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .is_ok()
    }
}

struct PreparedRead {
    gate: Option<Arc<CallbackGate>>,
    gate_bind: bool,
    gate_take: bool,
    reject_bind: bool,
    binds: AtomicUsize,
    take_requests: AtomicUsize,
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
        if self.gate_bind
            && let Some(gate) = &self.gate
            && !gate.pause("bind")
        {
            return Err(ExternalBindError::AlreadyBound);
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
    fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        Poll::Pending
    }
}

impl PreparedFsRead for PreparedRead {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.take_requests.fetch_add(1, Ordering::SeqCst);
        if self.gate_take
            && let Some(gate) = &self.gate
            && !gate.pause("take")
        {
            return None;
        }
        self.request.lock().unwrap().take()
    }
}

#[test]
fn test_bind_claim_prevents_request_recovery_without_waiting_for_provider() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let gate = Arc::new(CallbackGate {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let prepared = Arc::new(PreparedRead {
        gate: Some(Arc::clone(&gate)),
        gate_bind: true,
        gate_take: false,
        reject_bind: false,
        binds: AtomicUsize::new(0),
        take_requests: AtomicUsize::new(0),
        request: Mutex::new(Some(FsOwnedReadRequest {
            path: Path::parse("/bind-window").unwrap(),
            options: ReadAllOptions::default(),
        })),
    });
    let prepared_task: Arc<dyn PreparedFsRead> = Arc::clone(&prepared) as Arc<dyn PreparedFsRead>;
    let adapter = Arc::new(PreparedFsReadAdapter::new(prepared_task));
    let binding_adapter = Arc::clone(&adapter);
    let binding =
        std::thread::spawn(move || PreparedExternalOperation::bind(binding_adapter.as_ref(), fresh_operation_id()));
    let bind_stage = entered_rx.recv_timeout(Duration::from_secs(2));
    let recovery = if matches!(bind_stage, Ok("bind")) {
        let request_adapter = Arc::clone(&adapter);
        let (result_tx, result_rx) = mpsc::channel();
        let thread = std::thread::spawn(move || result_tx.send(request_adapter.take_request()).unwrap());
        Some((thread, result_rx.recv_timeout(Duration::from_secs(2))))
    } else {
        None
    };

    // Always release the provider callback before asserting, so a failed
    // stage notification cannot strand a worker thread.
    let _ = release_tx.send(());
    let bind_result = binding.join().unwrap();
    let recovered = recovery.map(|(thread, result)| {
        thread.join().unwrap();
        result
    });

    assert_eq!(
        bind_stage.unwrap_or_else(|error| panic!("bind stage did not start: {error}")),
        "bind"
    );
    bind_result.unwrap();
    assert_eq!(recovered.unwrap().expect("recovery completion stage"), None);
    assert_eq!(
        prepared.take_requests.load(Ordering::SeqCst),
        0,
        "provider take callback ran after bind claimed"
    );
}

#[test]
fn test_request_recovery_claim_rejects_bind_before_provider_callback() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let gate = Arc::new(CallbackGate {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    });
    let prepared = Arc::new(PreparedRead {
        gate: Some(gate),
        gate_bind: false,
        gate_take: true,
        reject_bind: false,
        binds: AtomicUsize::new(0),
        take_requests: AtomicUsize::new(0),
        request: Mutex::new(Some(FsOwnedReadRequest {
            path: Path::parse("/take-window").unwrap(),
            options: ReadAllOptions::default(),
        })),
    });
    let adapter = Arc::new(PreparedFsReadAdapter::new(
        Arc::clone(&prepared) as Arc<dyn PreparedFsRead>
    ));
    let take_adapter = Arc::clone(&adapter);
    let (take_tx, take_rx) = mpsc::channel();
    let taking = std::thread::spawn(move || take_tx.send(take_adapter.take_request()).unwrap());
    let take_stage = entered_rx.recv_timeout(Duration::from_secs(2));
    let bind_result = if matches!(take_stage, Ok("take")) {
        Some(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()))
    } else {
        None
    };

    // The take callback has a bounded fallback as well as an explicit release.
    // Release it before any assertion so failures leave no blocked worker.
    let _ = release_tx.send(());
    let taken = take_rx.recv_timeout(Duration::from_secs(2));
    taking.join().unwrap();

    assert_eq!(
        take_stage.unwrap_or_else(|error| panic!("take stage did not start: {error}")),
        "take"
    );
    assert!(bind_result.expect("bind attempt after take claim").is_err());
    assert_eq!(
        prepared.binds.load(Ordering::SeqCst),
        0,
        "provider bind callback ran after take claimed"
    );
    assert_eq!(prepared.take_requests.load(Ordering::SeqCst), 1);
    assert_eq!(
        taken.expect("take completion stage").unwrap().path.as_str(),
        "/take-window"
    );
}

fn adapter(reject_bind: bool) -> (Arc<PreparedFsReadAdapter>, Arc<PreparedRead>) {
    let prepared = Arc::new(PreparedRead {
        gate: None,
        gate_bind: false,
        gate_take: false,
        reject_bind,
        binds: AtomicUsize::new(0),
        take_requests: AtomicUsize::new(0),
        request: Mutex::new(Some(FsOwnedReadRequest {
            path: Path::parse("/claim-contract").unwrap(),
            options: ReadAllOptions::default(),
        })),
    });
    let trait_object: Arc<dyn PreparedFsRead> = Arc::clone(&prepared) as Arc<dyn PreparedFsRead>;
    (Arc::new(PreparedFsReadAdapter::new(trait_object)), prepared)
}

#[test]
fn test_rejected_bind_rolls_back_and_keeps_request_recoverable() {
    let (adapter, prepared) = adapter(true);
    assert!(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).is_err());
    assert_eq!(adapter.take_request().unwrap().path.as_str(), "/claim-contract");
    assert!(PreparedExternalOperation::bind(adapter.as_ref(), fresh_operation_id()).is_err());
    assert_eq!(prepared.binds.load(Ordering::SeqCst), 1);
}

#[test]
fn test_successful_bind_prevents_recovery_and_repeated_provider_claim() {
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
            fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<$output, FsOperationFailure>> {
                Poll::Pending
            }
        }

        impl $spi for $prepared {
            fn take_unclaimed_request(&self) -> Option<$request> {
                self.request_reads.fetch_add(1, Ordering::SeqCst);
                None
            }
            fn install_effect_reservation(&self, reservation: FsEffectReservation) -> Result<(), FsEffectReservation> {
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
    CopyOutcome,
    FsOwnedCopyRequest,
    copy_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedRenameFixture,
    PreparedFsRename,
    PreparedFsRenameAdapter,
    RenameOutcome,
    FsOwnedRenameRequest,
    rename_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedTreePublishFixture,
    PreparedFsTreePublish,
    PreparedFsTreePublishAdapter,
    TreePublishOutcome,
    FsOwnedTreePublishRequest,
    tree_publish_claim_blocks_recovery_and_repeated_bind
);
mutation_claim_contract!(
    PreparedWriteFixture,
    PreparedFsWrite,
    PreparedFsWriteAdapter,
    WriteOutcome,
    FsOwnedWriteRequest,
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
