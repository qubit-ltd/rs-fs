// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0 (the "License");
//    you may not use this file except in compliance with the License.
// =============================================================================
//! Public parent-bound submission entry points preserve admission boundaries
//! and owned requests.

use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsEffectFact;
use qubit_fs::FsEffectReservation;
use qubit_fs::FsEffectSlotPool;
use qubit_fs::FsEffectState;
use qubit_fs::FsError;
use qubit_fs::FsExecutionBinding;
use qubit_fs::FsOperationFailure;
use qubit_fs::FsOperationFailureKind;
use qubit_fs::FsOwnedCopyRequest;
use qubit_fs::FsOwnedReadRequest;
use qubit_fs::FsOwnedRenameRequest;
use qubit_fs::FsOwnedTreePublishRequest;
use qubit_fs::FsOwnedWriteRequest;
use qubit_fs::FsPrepareFailure;
use qubit_fs::FsPrepareFailureKind;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyMethod;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::copy::CopyStats;
use qubit_fs::directory::TreePublishOutcome;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation as FsOperationKind;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::TreeValidationOptions;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadAllOptions;
use qubit_fs::rename::RenameFailureState;
use qubit_fs::rename::RenameOptions;
use qubit_fs::rename::RenameOutcome;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ManagedFsMutationSpi;
use qubit_fs::spi::ManagedFsReadSpi;
use qubit_fs::spi::ManagedFsTreeSpi;
use qubit_fs::spi::PreparedFsCopy;
use qubit_fs::spi::PreparedFsRead;
use qubit_fs::spi::PreparedFsRename;
use qubit_fs::spi::PreparedFsTreePublish;
use qubit_fs::spi::PreparedFsWrite;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
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
use qubit_operation::StopReason;

struct FacadeSpi;

impl FileSystemSpi for FacadeSpi {
    fn properties(&self) -> ProviderProperties {
        properties()
    }

    fn stat(&self, _: StatRequest<'_>) -> qubit_fs::FsResult<StatResponse> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperationKind::Stat,
            "unused",
        ))
    }
}

impl AsyncFileSystemSpi for FacadeSpi {
    fn properties(&self) -> ProviderProperties {
        properties()
    }

    fn stat<'a>(
        &'a self,
        _: StatRequest<'a>,
    ) -> qubit_fs::spi::SpiFuture<'a, qubit_fs::FsResult<StatResponse>> {
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                FsOperationKind::Stat,
                "unused",
            ))
        })
    }
}

fn properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("managed-submission-entry-tests").unwrap(),
            "managed-submission-entry-tests",
            PathSemantics::Hierarchical,
        ),
        Default::default(),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .unwrap()
}

struct RejectProviders;

impl ManagedFsReadSpi for RejectProviders {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("test".into()),
            request,
        })
    }
}

impl ManagedFsMutationSpi for RejectProviders {
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>>
    {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("test".into()),
            request,
        })
    }

    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>> {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("test".into()),
            request,
        })
    }

    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>>
    {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("test".into()),
            request,
        })
    }
}

impl ManagedFsTreeSpi for RejectProviders {
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<
        Arc<dyn qubit_fs::spi::PreparedFsTreePublish>,
        FsPrepareFailure<FsOwnedTreePublishRequest>,
    > {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("test".into()),
            request,
        })
    }
}

#[derive(Default)]
struct MutationProbe {
    prepared: AtomicUsize,
    bound: AtomicUsize,
    activated: AtomicUsize,
    recovered: AtomicUsize,
    stop_reasons: Mutex<Vec<StopReason>>,
}

struct SuccessfulProvider(Arc<MutationProbe>);

fn read_settlement(id: OperationId) -> SettlementReport {
    SettlementReport {
        operation_id: id,
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

struct SuccessfulPrepared<T: Send + 'static, R: Send + 'static> {
    request: Mutex<Option<R>>,
    id: Mutex<Option<OperationId>>,
    result: Mutex<Option<T>>,
    failure: Mutex<Option<FsOperationFailure>>,
    outcome: Mutex<Option<OperationOutcome<T, FsOperationFailure>>>,
    reservation: Mutex<Option<FsEffectReservation>>,
    effect_fact: Option<FsEffectFact>,
    completed: AtomicBool,
    failed: AtomicBool,
    waker: Mutex<Option<Waker>>,
    probe: Arc<MutationProbe>,
}

impl<T: Send + 'static, R: Send + 'static> SuccessfulPrepared<T, R> {
    fn new(
        request: R,
        result: T,
        effect_fact: Option<FsEffectFact>,
        probe: Arc<MutationProbe>,
    ) -> Self {
        Self {
            request: Mutex::new(Some(request)),
            id: Mutex::new(None),
            result: Mutex::new(Some(result)),
            failure: Mutex::new(None),
            outcome: Mutex::new(None),
            reservation: Mutex::new(None),
            effect_fact,
            completed: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            waker: Mutex::new(None),
            probe,
        }
    }

    fn new_failed(
        request: R,
        failure: FsOperationFailure,
        effect_fact: FsEffectFact,
        probe: Arc<MutationProbe>,
    ) -> Self {
        Self {
            request: Mutex::new(Some(request)),
            id: Mutex::new(None),
            result: Mutex::new(None),
            failure: Mutex::new(Some(failure)),
            outcome: Mutex::new(None),
            reservation: Mutex::new(None),
            effect_fact: Some(effect_fact),
            completed: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            waker: Mutex::new(None),
            probe,
        }
    }

    fn finish(&self) {
        let mut outcome = self.outcome.lock().unwrap();
        if outcome.is_some() {
            return;
        }
        if let Some(reservation) = self.reservation.lock().unwrap().take() {
            reservation.publish(
                self.effect_fact
                    .clone()
                    .unwrap_or(FsEffectFact::State(FsEffectState::Applied)),
            );
        }
        let completed_outcome = match self.result.lock().unwrap().take() {
            Some(result) => OperationOutcome::Succeeded(result),
            None => {
                self.failed.store(true, Ordering::SeqCst);
                OperationOutcome::Failed(
                    self.failure
                        .lock()
                        .unwrap()
                        .take()
                        .expect("failed operation retains its typed failure"),
                )
            }
        };
        *outcome = Some(completed_outcome);
        self.completed.store(true, Ordering::SeqCst);
        drop(outcome);
        if let Some(waker) = self.waker.lock().unwrap().take() {
            waker.wake();
        }
    }

    fn take_request(&self) -> Option<R> {
        if self.id.lock().unwrap().is_some() {
            return None;
        }
        self.reservation.lock().unwrap().take();
        self.probe.recovered.fetch_add(1, Ordering::SeqCst);
        self.request.lock().unwrap().take()
    }

    fn install_reservation(
        &self,
        reservation: FsEffectReservation,
    ) -> Result<(), FsEffectReservation> {
        let mut stored = self.reservation.lock().unwrap();
        if stored.is_some() || self.completed.load(Ordering::SeqCst) {
            Err(reservation)
        } else {
            *stored = Some(reservation);
            Ok(())
        }
    }
}

impl<T: Send + 'static, R: Send + 'static> OperationControl for SuccessfulPrepared<T, R> {
    fn request_stop(&self, reason: StopReason) {
        self.probe.stop_reasons.lock().unwrap().push(reason);
        self.finish();
    }

    fn snapshot(&self) -> ChildSnapshot {
        let id = self
            .id
            .lock()
            .unwrap()
            .expect("managed operation was bound");
        let completed = self.completed.load(Ordering::SeqCst);
        ChildSnapshot {
            operation_id: id,
            execution: if completed {
                ExecutionPhase::Finished
            } else {
                ExecutionPhase::Running
            },
            stop: None,
            outcome: completed.then_some(ChildOutcomeSummary {
                operation_id: id,
                kind: if self.failed.load(Ordering::SeqCst) {
                    ChildOutcomeKind::Failed
                } else {
                    ChildOutcomeKind::Succeeded
                },
                summary: DiagnosticSummary::new("test mutation completed"),
            }),
            input_released: completed,
            settlement: completed.then(|| Arc::new(read_settlement(id))),
        }
    }

    fn poll_outcome(&self, cx: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(self.snapshot().outcome.unwrap())
        } else {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_release(&self, cx: &mut Context<'_>) -> Poll<ChildRelease> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(ChildRelease {
                operation_id: self.id.lock().unwrap().unwrap(),
                confirmed_at: None,
            })
        } else {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }

    fn poll_settlement(&self, cx: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        if self.completed.load(Ordering::SeqCst) {
            Poll::Ready(Arc::new(read_settlement(self.id.lock().unwrap().unwrap())))
        } else {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl<T: Send + 'static, R: Send + 'static> PreparedExternalOperation<T, FsOperationFailure>
    for SuccessfulPrepared<T, R>
{
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        let mut bound = self.id.lock().unwrap();
        if bound.is_some() {
            return Err(ExternalBindError::AlreadyBound);
        }
        *bound = Some(id);
        self.probe.bound.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn activate(self: Arc<Self>, _: OperationScope) -> Result<(), ExecutionFailure> {
        self.probe.activated.fetch_add(1, Ordering::SeqCst);
        if self.failure.lock().unwrap().is_some() {
            self.finish();
        }
        Ok(())
    }

    fn abandon(self: Arc<Self>) {
        self.reservation.lock().unwrap().take();
    }

    fn poll_take_outcome(
        &self,
        cx: &mut Context<'_>,
    ) -> Poll<OperationOutcome<T, FsOperationFailure>> {
        let outcome = self.outcome.lock().unwrap().take();
        if outcome.is_some() {
            Poll::Ready(outcome.unwrap())
        } else {
            *self.waker.lock().unwrap() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

// Each prepared trait adapter delegates to the same ownership primitive.
impl PreparedFsRead for SuccessfulPrepared<Vec<u8>, FsOwnedReadRequest> {
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest> {
        self.take_request()
    }
}
impl PreparedFsWrite for SuccessfulPrepared<WriteOutcome, FsOwnedWriteRequest> {
    fn take_unclaimed_request(&self) -> Option<FsOwnedWriteRequest> {
        self.take_request()
    }
    fn install_effect_reservation(
        &self,
        reservation: FsEffectReservation,
    ) -> Result<(), FsEffectReservation> {
        self.install_reservation(reservation)
    }
}
impl PreparedFsCopy for SuccessfulPrepared<CopyOutcome, FsOwnedCopyRequest> {
    fn take_unclaimed_request(&self) -> Option<FsOwnedCopyRequest> {
        self.take_request()
    }
    fn install_effect_reservation(
        &self,
        reservation: FsEffectReservation,
    ) -> Result<(), FsEffectReservation> {
        self.install_reservation(reservation)
    }
}
impl PreparedFsRename for SuccessfulPrepared<RenameOutcome, FsOwnedRenameRequest> {
    fn take_unclaimed_request(&self) -> Option<FsOwnedRenameRequest> {
        self.take_request()
    }
    fn install_effect_reservation(
        &self,
        reservation: FsEffectReservation,
    ) -> Result<(), FsEffectReservation> {
        self.install_reservation(reservation)
    }
}
impl PreparedFsTreePublish for SuccessfulPrepared<TreePublishOutcome, FsOwnedTreePublishRequest> {
    fn take_unclaimed_request(&self) -> Option<FsOwnedTreePublishRequest> {
        self.take_request()
    }
    fn install_effect_reservation(
        &self,
        reservation: FsEffectReservation,
    ) -> Result<(), FsEffectReservation> {
        self.install_reservation(reservation)
    }
}

impl ManagedFsMutationSpi for SuccessfulProvider {
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        let bytes = request.contents.len() as u64;
        Ok(Arc::new(SuccessfulPrepared::new(
            request,
            WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct)
                .with_bytes_written(bytes),
            Some(FsEffectFact::State(FsEffectState::Applied)),
            Arc::clone(&self.0),
        )))
    }

    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(SuccessfulPrepared::new(
            request,
            CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Native,
                AchievedAtomicity::NonAtomic,
            ),
            Some(FsEffectFact::State(FsEffectState::Applied)),
            Arc::clone(&self.0),
        )))
    }

    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        let outcome = RenameOutcome::new(
            request.source.clone(),
            request.destination.clone(),
            AchievedAtomicity::NonAtomic,
            PublicationMethod::Direct,
        );
        Ok(Arc::new(SuccessfulPrepared::new(
            request,
            outcome,
            Some(FsEffectFact::State(FsEffectState::Applied)),
            Arc::clone(&self.0),
        )))
    }
}

impl ManagedFsReadSpi for SuccessfulProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(SuccessfulPrepared::new(
            request,
            b"read".to_vec(),
            None,
            Arc::clone(&self.0),
        )))
    }
}

impl ManagedFsTreeSpi for SuccessfulProvider {
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        let outcome = TreePublishOutcome {
            entries_published: 1,
            content_bytes: 7,
            atomicity: AchievedAtomicity::NonAtomic,
            method: PublicationMethod::Direct,
            durable: false,
        };
        Ok(Arc::new(SuccessfulPrepared::new(
            request,
            outcome.clone(),
            Some(FsEffectFact::TreePublish {
                outcome: Arc::new(outcome),
                recovery: None,
            }),
            Arc::clone(&self.0),
        )))
    }
}

struct FailureProvider(Arc<MutationProbe>);

impl ManagedFsMutationSpi for FailureProvider {
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>> {
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Unsupported,
            request,
        })
    }

    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        let fact_error = Arc::new(FsError::new(
            FsErrorKind::Io,
            FsOperationKind::Copy,
            "copy failed after claim",
        ));
        let operation_error = Arc::new(FsError::new(
            FsErrorKind::Io,
            FsOperationKind::Copy,
            "copy failed after claim",
        ));
        let fact = FsEffectFact::CopyFailure {
            error: fact_error,
            state: CopyFailureState::PartiallyPublished,
            partial_stats: CopyStats::default(),
            recovery: None,
        };
        let failure = FsOperationFailure::new(
            FsOperationFailureKind::Provider(operation_error),
            FsEffectState::Indeterminate,
        );
        Ok(Arc::new(SuccessfulPrepared::new_failed(
            request,
            failure,
            fact,
            Arc::clone(&self.0),
        )))
    }

    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>> {
        self.0.prepared.fetch_add(1, Ordering::SeqCst);
        let fact_error = Arc::new(FsError::new(
            FsErrorKind::Io,
            FsOperationKind::Rename,
            "rename failed after claim",
        ));
        let operation_error = Arc::new(FsError::new(
            FsErrorKind::Io,
            FsOperationKind::Rename,
            "rename failed after claim",
        ));
        let state = RenameFailureState::Renamed;
        let fact = FsEffectFact::RenameFailure {
            error: fact_error,
            state,
        };
        let failure = FsOperationFailure::new(
            FsOperationFailureKind::Provider(operation_error),
            FsEffectState::Applied,
        );
        Ok(Arc::new(SuccessfulPrepared::new_failed(
            request,
            failure,
            fact,
            Arc::clone(&self.0),
        )))
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::yield_now();
    }
}

fn path(value: &str) -> Path {
    Path::parse(value).unwrap()
}

fn copy_request(source: &str, destination: &str) -> FsOwnedCopyRequest {
    FsOwnedCopyRequest {
        source: path(source),
        destination: path(destination),
        options: CopyOptions::file(),
    }
}

fn rename_request(source: &str, destination: &str) -> FsOwnedRenameRequest {
    FsOwnedRenameRequest {
        source: path(source),
        destination: path(destination),
        options: RenameOptions::default(),
    }
}

fn tree_request() -> FsOwnedTreePublishRequest {
    FsOwnedTreePublishRequest {
        staging_root: path("/stage"),
        destination_root: path("/destination"),
        validation: TreeValidationOptions::default(),
        validated_version: None,
        publication: Default::default(),
    }
}

fn take_error<T, R>(
    result: Result<T, qubit_fs::FsSubmitFailure<R>>,
) -> qubit_fs::FsSubmitFailure<R> {
    match result {
        Ok(_) => panic!("test provider unexpectedly accepted preparation"),
        Err(error) => error,
    }
}

trait ExpectAdmitted<T> {
    fn expect_admitted(self, message: &str) -> T;
}

impl<T, R> ExpectAdmitted<T> for Result<T, qubit_fs::FsSubmitFailure<R>> {
    fn expect_admitted(self, message: &str) -> T {
        match self {
            Ok(value) => value,
            Err(_) => panic!("{message}"),
        }
    }
}

#[test]
fn test_both_facades_submit_all_parent_entries_under_the_active_parent() {
    let sync = FileSystem::from_spi(FacadeSpi).unwrap();
    let asynchronous = AsyncFileSystem::from_spi(FacadeSpi).unwrap();
    let effects = FsEffectSlotPool::new(8);
    let probe = Arc::new(MutationProbe::default());
    let ids = Arc::new(Mutex::new(Vec::with_capacity(10)));
    let effect_observers = Arc::new(Mutex::new(Vec::with_capacity(8)));
    let (ready_tx, ready_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let mut parent = manager
        .try_submit(Default::default(), {
            let ids = Arc::clone(&ids);
            let effect_observers = Arc::clone(&effect_observers);
            let provider = SuccessfulProvider(Arc::clone(&probe));
            move |scope| {
                let mut held: Vec<Box<dyn Send>> = Vec::with_capacity(10);
                let child = sync
                    .submit_read_all_in(
                        &scope,
                        &provider,
                        path("/sync/read"),
                        ReadAllOptions::default(),
                    )
                    .expect_admitted("sync read is admitted beneath its parent");
                ids.lock().unwrap().push(child.id());
                held.push(Box::new(child));
                let child = asynchronous
                    .submit_read_all_in(
                        &scope,
                        &provider,
                        path("/async/read"),
                        ReadAllOptions::default(),
                    )
                    .expect_admitted("async read is admitted beneath its parent");
                ids.lock().unwrap().push(child.id());
                held.push(Box::new(child));

                let child = sync
                    .submit_write_in(
                        &scope,
                        &provider,
                        FsOwnedWriteRequest {
                            path: path("/sync/write"),
                            contents: b"sync".to_vec(),
                            options: Default::default(),
                        },
                        &effects,
                    )
                    .expect_admitted("sync write is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = sync
                    .submit_copy_in(
                        &scope,
                        &provider,
                        copy_request("/sync/src", "/sync/dst"),
                        &effects,
                    )
                    .expect_admitted("sync copy is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = sync
                    .submit_rename_in(
                        &scope,
                        &provider,
                        rename_request("/sync/old", "/sync/new"),
                        &effects,
                    )
                    .expect_admitted("sync rename is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = sync
                    .submit_tree_publish_in(&scope, &provider, tree_request(), &effects)
                    .expect_admitted("sync tree publication is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((true, child.effect.clone()));
                held.push(Box::new(child));

                let child = asynchronous
                    .submit_write_in(
                        &scope,
                        &provider,
                        FsOwnedWriteRequest {
                            path: path("/async/write"),
                            contents: b"async".to_vec(),
                            options: Default::default(),
                        },
                        &effects,
                    )
                    .expect_admitted("async write is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = asynchronous
                    .submit_copy_in(
                        &scope,
                        &provider,
                        copy_request("/async/src", "/async/dst"),
                        &effects,
                    )
                    .expect_admitted("async copy is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = asynchronous
                    .submit_rename_in(
                        &scope,
                        &provider,
                        rename_request("/async/old", "/async/new"),
                        &effects,
                    )
                    .expect_admitted("async rename is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((false, child.effect.clone()));
                held.push(Box::new(child));
                let child = asynchronous
                    .submit_tree_publish_in(&scope, &provider, tree_request(), &effects)
                    .expect_admitted("async tree publication is admitted beneath its parent");
                ids.lock().unwrap().push(child.operation.id());
                effect_observers
                    .lock()
                    .unwrap()
                    .push((true, child.effect.clone()));
                held.push(Box::new(child));

                ready_tx.send(()).unwrap();
                resume_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("test stop gate should open");
                assert_eq!(
                    scope.request_stop(StopReason::UserCancelled),
                    qubit_operation::StopRequestStatus::Accepted,
                );
                Ok::<_, ()>(())
            }
        })
        .unwrap();

    ready_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("parent should submit ten children");
    resume_tx.send(()).unwrap();
    let _ = parent.wait_outcome().unwrap();
    assert_eq!(
        probe.stop_reasons.lock().unwrap().len(),
        10,
        "the active parent scope stop reaches each accepted facade child"
    );
    let child_ids = ids.lock().unwrap().clone();
    assert_eq!(child_ids.len(), 10);
    let settlement = parent.wait_settlement().unwrap();
    assert!(child_ids.iter().all(|id| {
        settlement
            .children
            .iter()
            .any(|child| child.operation_id == *id)
    }));
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 10);
    assert_eq!(probe.bound.load(Ordering::SeqCst), 10);
    assert_eq!(probe.activated.load(Ordering::SeqCst), 10);
    assert_eq!(probe.recovered.load(Ordering::SeqCst), 0);
    for (tree_publish, observer) in effect_observers.lock().unwrap().iter() {
        let fact = observer
            .wait_final()
            .expect("accepted mutation must publish its final effect");
        if *tree_publish {
            assert!(matches!(fact, FsEffectFact::TreePublish { .. }));
        } else {
            assert!(matches!(fact, FsEffectFact::State(FsEffectState::Applied)));
        }
    }
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn test_closed_scope_returns_successfully_prepared_request_before_claim_or_activation() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let mut parent = manager
        .try_submit(Default::default(), |scope| {
            Ok::<_, ()>(FsExecutionBinding::new(scope.manager()))
        })
        .unwrap();
    let binding = match parent.wait_outcome().unwrap() {
        OperationOutcome::Succeeded(binding) => binding,
        _ => panic!("binding parent should complete successfully"),
    };
    let probe = Arc::new(MutationProbe::default());
    let provider = SuccessfulProvider(Arc::clone(&probe));
    let effects = FsEffectSlotPool::new(4);
    let failure = take_error(binding.submit_read_all(
        &provider,
        FsOwnedReadRequest {
            path: path("/closed/original"),
            options: ReadAllOptions::default(),
        },
    ));
    let (admission, preparation, request) = failure.into_parts();
    assert_eq!(admission, Some(qubit_operation::AdmissionError::ScopeEnded));
    assert!(preparation.is_none());
    assert_eq!(request.path.as_str(), "/closed/original");
    let (admission, preparation, request) = take_error(binding.submit_copy(
        &provider,
        copy_request("/closed/copy-source", "/closed/copy-destination"),
        &effects,
    ))
    .into_parts();
    assert_eq!(admission, Some(qubit_operation::AdmissionError::ScopeEnded));
    assert!(preparation.is_none());
    assert_eq!(request.source.as_str(), "/closed/copy-source");
    assert_eq!(request.destination.as_str(), "/closed/copy-destination");
    let (admission, preparation, request) = take_error(binding.submit_rename(
        &provider,
        rename_request("/closed/rename-source", "/closed/rename-destination"),
        &effects,
    ))
    .into_parts();
    assert_eq!(admission, Some(qubit_operation::AdmissionError::ScopeEnded));
    assert!(preparation.is_none());
    assert_eq!(request.source.as_str(), "/closed/rename-source");
    assert_eq!(request.destination.as_str(), "/closed/rename-destination");
    let (admission, preparation, request) = take_error(binding.submit_copy_async(
        &provider,
        copy_request(
            "/closed/async-copy-source",
            "/closed/async-copy-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert_eq!(admission, Some(qubit_operation::AdmissionError::ScopeEnded));
    assert!(preparation.is_none());
    assert_eq!(request.source.as_str(), "/closed/async-copy-source");
    assert_eq!(
        request.destination.as_str(),
        "/closed/async-copy-destination"
    );
    let (admission, preparation, request) = take_error(binding.submit_rename_async(
        &provider,
        rename_request(
            "/closed/async-rename-source",
            "/closed/async-rename-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert_eq!(admission, Some(qubit_operation::AdmissionError::ScopeEnded));
    assert!(preparation.is_none());
    assert_eq!(request.source.as_str(), "/closed/async-rename-source");
    assert_eq!(
        request.destination.as_str(),
        "/closed/async-rename-destination"
    );
    assert_eq!(probe.prepared.load(Ordering::SeqCst), 5);
    assert_eq!(
        probe.recovered.load(Ordering::SeqCst),
        5,
        "the prepared provider returns the original unclaimed request"
    );
    assert_eq!(
        probe.bound.load(Ordering::SeqCst),
        0,
        "a closed scope cannot claim the prepared operation"
    );
    assert_eq!(
        probe.activated.load(Ordering::SeqCst),
        0,
        "a closed scope cannot activate the prepared operation"
    );
    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}

#[test]
fn test_binding_copy_and_rename_sync_async_preserve_prepare_failures_and_requests() {
    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let binding = FsExecutionBinding::new(manager.clone());
    let providers = RejectProviders;
    let effects = FsEffectSlotPool::new(4);

    let (admission, preparation, request) = take_error(binding.submit_copy(
        &providers,
        copy_request(
            "/binding/sync-copy-source",
            "/binding/sync-copy-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert!(admission.is_none());
    assert_eq!(
        preparation,
        Some(FsPrepareFailureKind::Rejected("test".into()))
    );
    assert_eq!(request.source.as_str(), "/binding/sync-copy-source");
    assert_eq!(
        request.destination.as_str(),
        "/binding/sync-copy-destination"
    );

    let (admission, preparation, request) = take_error(binding.submit_rename(
        &providers,
        rename_request(
            "/binding/sync-rename-source",
            "/binding/sync-rename-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert!(admission.is_none());
    assert_eq!(
        preparation,
        Some(FsPrepareFailureKind::Rejected("test".into()))
    );
    assert_eq!(request.source.as_str(), "/binding/sync-rename-source");
    assert_eq!(
        request.destination.as_str(),
        "/binding/sync-rename-destination"
    );

    let (admission, preparation, request) = take_error(binding.submit_copy_async(
        &providers,
        copy_request(
            "/binding/async-copy-source",
            "/binding/async-copy-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert!(admission.is_none());
    assert_eq!(
        preparation,
        Some(FsPrepareFailureKind::Rejected("test".into()))
    );
    assert_eq!(request.source.as_str(), "/binding/async-copy-source");
    assert_eq!(
        request.destination.as_str(),
        "/binding/async-copy-destination"
    );

    let (admission, preparation, request) = take_error(binding.submit_rename_async(
        &providers,
        rename_request(
            "/binding/async-rename-source",
            "/binding/async-rename-destination",
        ),
        &effects,
    ))
    .into_parts();
    assert!(admission.is_none());
    assert_eq!(
        preparation,
        Some(FsPrepareFailureKind::Rejected("test".into()))
    );
    assert_eq!(request.source.as_str(), "/binding/async-rename-source");
    assert_eq!(
        request.destination.as_str(),
        "/binding/async-rename-destination"
    );

    let success_probe = Arc::new(MutationProbe::default());
    let success_provider = SuccessfulProvider(Arc::clone(&success_probe));
    let copy = binding
        .submit_copy(
            &success_provider,
            copy_request(
                "/binding/success-copy-source",
                "/binding/success-copy-target",
            ),
            &effects,
        )
        .expect_admitted("sync copy binding should admit a successful prepared operation");
    assert_eq!(
        copy.operation.request_stop(StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted
    );
    assert!(matches!(
        copy.effect.wait_final().unwrap(),
        FsEffectFact::State(FsEffectState::Applied)
    ));

    let rename = binding
        .submit_rename(
            &success_provider,
            rename_request(
                "/binding/success-rename-source",
                "/binding/success-rename-target",
            ),
            &effects,
        )
        .expect_admitted("sync rename binding should admit a successful prepared operation");
    assert_eq!(
        rename.operation.request_stop(StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted
    );
    assert!(matches!(
        rename.effect.wait_final().unwrap(),
        FsEffectFact::State(FsEffectState::Applied)
    ));

    let copy = binding
        .submit_copy_async(
            &success_provider,
            copy_request(
                "/binding/success-async-copy-source",
                "/binding/success-async-copy-target",
            ),
            &effects,
        )
        .expect_admitted("async copy binding should admit a successful prepared operation");
    assert_eq!(
        copy.operation.request_stop(StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted
    );
    assert!(matches!(
        copy.effect.wait_final().unwrap(),
        FsEffectFact::State(FsEffectState::Applied)
    ));

    let rename = binding
        .submit_rename_async(
            &success_provider,
            rename_request(
                "/binding/success-async-rename-source",
                "/binding/success-async-rename-target",
            ),
            &effects,
        )
        .expect_admitted("async rename binding should admit a successful prepared operation");
    assert_eq!(
        rename.operation.request_stop(StopReason::UserCancelled),
        qubit_operation::StopRequestStatus::Accepted
    );
    assert!(matches!(
        rename.effect.wait_final().unwrap(),
        FsEffectFact::State(FsEffectState::Applied)
    ));
    assert_eq!(success_probe.prepared.load(Ordering::SeqCst), 4);
    assert_eq!(success_probe.bound.load(Ordering::SeqCst), 4);
    assert_eq!(success_probe.activated.load(Ordering::SeqCst), 4);

    let failure_probe = Arc::new(MutationProbe::default());
    let failure_provider = FailureProvider(Arc::clone(&failure_probe));
    let mut copy = binding
        .submit_copy(
            &failure_provider,
            copy_request("/binding/failed-copy-source", "/binding/failed-copy-target"),
            &effects,
        )
        .expect_admitted("sync copy failure is returned after claim");
    assert!(matches!(
        copy.operation.wait_outcome().unwrap(),
        OperationOutcome::Failed(_)
    ));
    assert!(matches!(
        copy.effect.wait_final().unwrap(),
        FsEffectFact::CopyFailure {
            state: CopyFailureState::PartiallyPublished,
            ..
        }
    ));

    let mut rename = binding
        .submit_rename(
            &failure_provider,
            rename_request(
                "/binding/failed-rename-source",
                "/binding/failed-rename-target",
            ),
            &effects,
        )
        .expect_admitted("sync rename failure is returned after claim");
    assert!(matches!(
        rename.operation.wait_outcome().unwrap(),
        OperationOutcome::Failed(_)
    ));
    assert!(matches!(
        rename.effect.wait_final().unwrap(),
        FsEffectFact::RenameFailure {
            state: RenameFailureState::Renamed,
            ..
        }
    ));

    let mut copy = binding
        .submit_copy_async(
            &failure_provider,
            copy_request(
                "/binding/failed-async-copy-source",
                "/binding/failed-async-copy-target",
            ),
            &effects,
        )
        .expect_admitted("async copy failure is returned after claim");
    assert!(matches!(
        block_on(copy.operation.wait_outcome()).unwrap(),
        OperationOutcome::Failed(_)
    ));
    assert!(matches!(
        copy.effect.wait_final().unwrap(),
        FsEffectFact::CopyFailure {
            state: CopyFailureState::PartiallyPublished,
            ..
        }
    ));

    let mut rename = binding
        .submit_rename_async(
            &failure_provider,
            rename_request(
                "/binding/failed-async-rename-source",
                "/binding/failed-async-rename-target",
            ),
            &effects,
        )
        .expect_admitted("async rename failure is returned after claim");
    assert!(matches!(
        block_on(rename.operation.wait_outcome()).unwrap(),
        OperationOutcome::Failed(_)
    ));
    assert!(matches!(
        rename.effect.wait_final().unwrap(),
        FsEffectFact::RenameFailure {
            state: RenameFailureState::Renamed,
            ..
        }
    ));
    assert_eq!(failure_probe.prepared.load(Ordering::SeqCst), 4);
    assert_eq!(failure_probe.bound.load(Ordering::SeqCst), 4);
    assert_eq!(failure_probe.activated.load(Ordering::SeqCst), 4);

    let _ = manager
        .request_shutdown(qubit_operation::ShutdownMode::Drain)
        .wait()
        .unwrap();
}
