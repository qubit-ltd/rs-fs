// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;

use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOperationFailure;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::FsPrepareFailureKind;
use qubit_fs::Path;
use qubit_fs::ReadAllOptions;
use qubit_fs::spi::ManagedFsReadSpi;
use qubit_fs::spi::PreparedFsRead;
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
use qubit_operation::OperationOutcome;
use qubit_operation::OperationScope;
use qubit_operation::PreparedExternalOperation;
use qubit_operation::SettlementReport;
use qubit_operation::SettlementStatus;
use qubit_operation::ShutdownMode;
use qubit_operation::StopReason;

#[derive(Default)]
pub struct MemoryReadProvider {
    reads: Arc<AtomicBool>,
    reject: bool,
}

impl MemoryReadProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rejecting() -> Self {
        Self {
            reads: Arc::new(AtomicBool::new(false)),
            reject: true,
        }
    }

    pub fn io_started(&self) -> bool {
        self.reads.load(Ordering::SeqCst)
    }
}

impl ManagedFsReadSpi for MemoryReadProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        if self.reject {
            return Err(FsPrepareFailure {
                kind: FsPrepareFailureKind::Rejected("policy".into()),
                request,
            });
        }
        Ok(Arc::new(MemoryRead::new(request, Arc::clone(&self.reads))))
    }
}

struct MemoryRead {
    request: Mutex<Option<FsOwnedReadRequest>>,
    id: Mutex<Option<OperationId>>,
    outcome: Mutex<Option<OperationOutcome<Vec<u8>, FsOperationFailure>>>,
    done: AtomicBool,
    io_started: Arc<AtomicBool>,
}

impl MemoryRead {
    fn new(request: FsOwnedReadRequest, io_started: Arc<AtomicBool>) -> Self {
        Self {
            request: Mutex::new(Some(request)),
            id: Mutex::new(None),
            outcome: Mutex::new(None),
            done: AtomicBool::new(false),
            io_started,
        }
    }

    fn operation_id(&self) -> OperationId {
        self.id
            .lock()
            .expect("operation id mutex poisoned")
            .expect("operation is bound before observation")
    }

    fn finished(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }
}

impl OperationControl for MemoryRead {
    fn request_stop(&self, _: StopReason) {}

    fn snapshot(&self) -> ChildSnapshot {
        let operation_id = self.operation_id();
        let finished = self.finished();
        ChildSnapshot {
            operation_id,
            execution: if finished {
                ExecutionPhase::Finished
            } else {
                ExecutionPhase::Running
            },
            stop: None,
            outcome: finished.then(|| ChildOutcomeSummary {
                operation_id,
                kind: ChildOutcomeKind::Succeeded,
                summary: DiagnosticSummary::new("read completed"),
            }),
            input_released: finished,
            settlement: finished.then(|| Arc::new(settlement(operation_id))),
        }
    }

    fn poll_outcome(&self, _: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        let operation_id = self.operation_id();
        if self.finished() {
            Poll::Ready(ChildOutcomeSummary {
                operation_id,
                kind: ChildOutcomeKind::Succeeded,
                summary: DiagnosticSummary::new("read completed"),
            })
        } else {
            Poll::Pending
        }
    }

    fn poll_release(&self, _: &mut Context<'_>) -> Poll<ChildRelease> {
        if self.finished() {
            Poll::Ready(ChildRelease {
                operation_id: self.operation_id(),
                confirmed_at: None,
            })
        } else {
            Poll::Pending
        }
    }

    fn poll_settlement(&self, _: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        if self.finished() {
            Poll::Ready(Arc::new(settlement(self.operation_id())))
        } else {
            Poll::Pending
        }
    }
}

impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for MemoryRead {
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        *self.id.lock().expect("operation id mutex poisoned") = Some(id);
        Ok(())
    }

    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        self.io_started.store(true, Ordering::SeqCst);
        self.request.lock().expect("request mutex poisoned").take();
        *self.outcome.lock().expect("outcome mutex poisoned") =
            Some(OperationOutcome::Succeeded(b"report ready".to_vec()));
        self.done.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn abandon(self: Arc<Self>) {
        self.request.lock().expect("request mutex poisoned").take();
        self.done.store(true, Ordering::SeqCst);
    }

    fn poll_take_outcome(&self, _: &mut Context<'_>) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        self.outcome
            .lock()
            .expect("outcome mutex poisoned")
            .take()
            .map_or(Poll::Pending, Poll::Ready)
    }
}

impl PreparedFsRead for MemoryRead {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.request.lock().expect("request mutex poisoned").take()
    }
}

pub fn read_request() -> FsOwnedReadRequest {
    FsOwnedReadRequest {
        path: Path::parse("/report.txt").expect("report path is valid"),
        options: ReadAllOptions::default(),
    }
}

pub fn manager() -> OperationManager {
    OperationManager::new(ManagerOptions::default()).expect("manager creation succeeds")
}

pub fn binding(manager: &OperationManager) -> FsExecutionBinding {
    FsExecutionBinding::new(manager.clone())
}

pub fn close(manager: &OperationManager) {
    let _shutdown = manager
        .request_shutdown(ShutdownMode::Drain)
        .wait()
        .expect("manager shuts down");
}

fn settlement(operation_id: OperationId) -> SettlementReport {
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
