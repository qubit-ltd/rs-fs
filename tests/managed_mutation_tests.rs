// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;
#[cfg(feature = "async")]
use std::sync::Mutex;
#[cfg(feature = "async")]
use std::sync::atomic::AtomicUsize;
#[cfg(feature = "async")]
use std::sync::atomic::Ordering;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FsEffectFact;
use qubit_fs::FsEffectSlotPool;
use qubit_fs::FsEffectSnapshot;
use qubit_fs::FsEffectState;
#[cfg(feature = "async")]
use qubit_fs::FsResult;
#[cfg(feature = "async")]
use qubit_fs::Path;
use qubit_fs::copy::CopyFailureState;
#[cfg(feature = "async")]
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyStats;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::execution::FsEffectReservation;
use qubit_fs::metadata::ContentPrecondition;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
#[cfg(feature = "async")]
use qubit_fs::metadata::FileSystemId;
#[cfg(feature = "async")]
use qubit_fs::metadata::FileSystemInfo;
#[cfg(feature = "async")]
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::ResourceVersion;
#[cfg(feature = "async")]
use qubit_fs::metadata::SymlinkPolicy;
#[cfg(feature = "async")]
use qubit_fs::path::PathConstraints;
#[cfg(feature = "async")]
use qubit_fs::path::PathSemantics;
use qubit_fs::rename::RenameFailureState;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
#[cfg(feature = "async")]
use qubit_fs::spi::CopyAttempt;
#[cfg(feature = "async")]
use qubit_fs::spi::CopyRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenReaderRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenWriterRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncReader;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::PreparedFsCopy;
use qubit_fs::spi::PreparedFsRename;
use qubit_fs::spi::PreparedFsWrite;
#[cfg(feature = "async")]
use qubit_fs::spi::ProviderOperation;
#[cfg(feature = "async")]
use qubit_fs::spi::ProviderOperations;
#[cfg(feature = "async")]
use qubit_fs::spi::ProviderProperties;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiCopyFailure;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
#[cfg(feature = "async")]
use qubit_fs::spi::StatRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::StatResponse;
#[cfg(feature = "async")]
use qubit_fs::temp::AsyncTempFile;
#[cfg(feature = "async")]
use qubit_fs::temp::TempOptions;
#[cfg(feature = "async")]
use qubit_fs::temp::TempResourceState;
#[cfg(feature = "async")]
use qubit_fs::write::AsyncWriteAllOperationState;
#[cfg(feature = "async")]
use qubit_fs::write::AsyncWriterRecovery;
#[cfg(feature = "async")]
use qubit_fs::write::WriteDisposition;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WritePrecondition;
#[cfg(feature = "async")]
use qubit_operation::AsyncCleanup;
#[cfg(feature = "async")]
use qubit_operation::CleanupContext;
#[cfg(feature = "async")]
use qubit_operation::CleanupFuture;
#[cfg(feature = "async")]
use qubit_operation::CleanupOutcome;
#[cfg(feature = "async")]
use qubit_operation::ManagerOptions;
#[cfg(feature = "async")]
use qubit_operation::OperationManager;
#[cfg(feature = "async")]
use qubit_operation::OperationOptions;
#[cfg(feature = "async")]
use qubit_operation::ResourceManageErrorKind;
#[cfg(feature = "async")]
use qubit_operation::ShutdownMode;
#[cfg(feature = "async")]
use qubit_operation::StopReason;

#[cfg(feature = "async")]
use crate::common::async_recording_spi::AsyncCopyStage;
#[cfg(feature = "async")]
use crate::common::async_recording_spi::AsyncRecordingConfig;
#[cfg(feature = "async")]
use crate::common::async_recording_spi::async_recording_file_system;
#[cfg(feature = "async")]
use crate::common::poll_support::assert_pending;
#[cfg(feature = "async")]
use crate::common::poll_support::ready;

#[test]
fn test_effect_reservation_stays_owned_until_recovery_finishes() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("slot is initially available");
    let observer = reservation.observer();
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(pool.reserve().is_err());

    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(observer);
    assert!(
        pool.reserve().is_err(),
        "active owner retains the bounded slot"
    );
    drop(reservation);
    assert!(
        pool.reserve().is_ok(),
        "releasing the owner reclaims the slot"
    );
}

#[test]
fn test_prepared_mutations_accept_one_owned_effect_reservation() {
    let _: fn(&dyn PreparedFsWrite, FsEffectReservation) -> Result<(), FsEffectReservation> =
        PreparedFsWrite::install_effect_reservation;
    let _: fn(&dyn PreparedFsCopy, FsEffectReservation) -> Result<(), FsEffectReservation> =
        PreparedFsCopy::install_effect_reservation;
    let _: fn(&dyn PreparedFsRename, FsEffectReservation) -> Result<(), FsEffectReservation> =
        PreparedFsRename::install_effect_reservation;
}

#[test]
fn test_a_lost_effect_is_not_reported_as_unchanged() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish(FsEffectFact::State(FsEffectState::Indeterminate));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
}

#[test]
fn test_failure_publication_preserves_write_count_and_publication_state() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_write_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Write,
            "write failed",
        )),
        WriteFailureState::Published,
        23,
        None,
    );
    let FsEffectSnapshot::Final(FsEffectFact::WriteFailure {
        state,
        written_bytes,
        recovery,
        ..
    }) = observer.snapshot()
    else {
        panic!("typed write failure fact expected");
    };
    assert_eq!(WriteFailureState::Published, state);
    assert_eq!(23, written_bytes);
    assert_eq!(None, recovery);
}

#[test]
fn test_failure_publication_preserves_copy_counters_and_uncertainty() {
    let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_copy_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Copy,
            "copy failed",
        )),
        CopyFailureState::Indeterminate,
        CopyStats {
            files: 2,
            bytes: 91,
            ..CopyStats::default()
        },
        None,
    );
    let FsEffectSnapshot::Final(FsEffectFact::CopyFailure {
        state,
        partial_stats,
        recovery,
        ..
    }) = observer.snapshot()
    else {
        panic!("typed copy failure fact expected");
    };
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(2, partial_stats.files);
    assert_eq!(91, partial_stats.bytes);
    assert_eq!(None, recovery);
}

#[test]
fn test_failure_publication_preserves_rename_transition_state() {
    let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_rename_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Rename,
            "rename failed",
        )),
        RenameFailureState::Renamed,
    );
    let FsEffectSnapshot::Final(FsEffectFact::RenameFailure { state, .. }) = observer.snapshot()
    else {
        panic!("typed rename failure fact expected");
    };
    assert_eq!(RenameFailureState::Renamed, state);
}

#[test]
fn test_content_and_version_conditions_are_distinct_inputs() {
    let options = WriteOptions::default().with_precondition(WritePrecondition::IfMatch(
        ResourceVersion::new("revision-7"),
    ));
    let content = ContentPrecondition::Equals(b"expected bytes".to_vec());
    assert_eq!(
        &WritePrecondition::IfMatch(ResourceVersion::new("revision-7")),
        options.precondition()
    );
    assert_eq!(
        ContentPrecondition::Equals(b"expected bytes".to_vec()),
        content
    );
}

#[test]
fn test_write_content_condition_is_preserved_independently_from_version_condition() {
    let options = WriteOptions::default()
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v4")))
        .with_content_precondition(ContentPrecondition::Equals(b"old bytes".to_vec()));
    assert_eq!(
        &WritePrecondition::IfMatch(ResourceVersion::new("v4")),
        options.precondition()
    );
    assert_eq!(
        &ContentPrecondition::Equals(b"old bytes".to_vec()),
        options.content_precondition()
    );
}

#[test]
fn test_required_content_compare_needs_guaranteed_conditional_write() {
    let options =
        WriteOptions::default().with_content_precondition(ContentPrecondition::Equals(vec![1]));
    let conditional = FileSystemCapabilities::new()
        .with_conditional(FileSystemCapability::Write)
        .with_conditional(FileSystemCapability::ConditionalWrite);
    assert!(options.validate_against(conditional).is_err());
    let guaranteed = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Write)
        .with_guaranteed(FileSystemCapability::ConditionalWrite);
    assert!(options.validate_against(guaranteed).is_ok());
}

#[cfg(feature = "async")]
mod common;

#[cfg(feature = "async")]
#[test]
fn test_native_copy_partial_side_effect_never_opens_fallback_streams() {
    struct SideEffectCopyProvider {
        properties: ProviderProperties,
        fallback_opens: Arc<AtomicUsize>,
    }
    impl AsyncFileSystemSpi for SideEffectCopyProvider {
        fn properties(&self) -> ProviderProperties {
            self.properties.clone()
        }

        fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::NotFound,
                    FsOperation::Stat,
                    "not used",
                ))
            })
        }

        fn open_reader<'a>(
            &'a self,
            request: OpenReaderRequest<'a>,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
            let _ = request;
            self.fallback_opens.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::UnsupportedOperation,
                    FsOperation::OpenReader,
                    "fallback must not run",
                ))
            })
        }

        fn open_writer<'a>(
            &'a self,
            request: OpenWriterRequest<'a>,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            let _ = request;
            self.fallback_opens.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::UnsupportedOperation,
                    FsOperation::OpenWriter,
                    "fallback must not run",
                ))
            })
        }

        fn try_copy<'a>(
            &'a self,
            _: CopyRequest<'a>,
        ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
            Box::pin(async {
                Err(SpiCopyFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::Copy,
                        "native copy failed after writing bytes",
                    )
                    .with_effect_state(FsEffectState::PartiallyApplied),
                    CopyFailureState::PartiallyPublished,
                    CopyStats {
                        files: 1,
                        bytes: 7,
                        ..CopyStats::default()
                    },
                ))
            })
        }
    }

    let properties = ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("managed-mutation-native").unwrap(),
            "native-fake",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new()
            .with(ProviderOperation::TryCopy)
            .with(ProviderOperation::OpenReader)
            .with(ProviderOperation::OpenWriter),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Copy)
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::Write),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .unwrap();
    let fallback_opens = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(SideEffectCopyProvider {
        properties,
        fallback_opens: Arc::clone(&fallback_opens),
    })
    .unwrap();
    let mut operation = filesystem
        .begin_copy(
            Path::parse("/source").unwrap(),
            Path::parse("/target").unwrap(),
            CopyOptions::default(),
        )
        .unwrap();
    let failure = ready(operation.execute()).expect_err("native failure remains failure");
    assert_eq!(CopyFailureState::PartiallyPublished, failure.state());
    assert_eq!(7, failure.partial_stats().bytes);
    assert_eq!(0, fallback_opens.load(Ordering::SeqCst));
}

#[cfg(feature = "async")]
#[test]
fn test_append_cancellation_keeps_acknowledged_bytes_and_writer_owner() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        pending_stage: Some(AsyncCopyStage::WriterFlush),
        writer_direct: true,
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/append").unwrap(),
            b"suffix".to_vec(),
            WriteOptions::default().with_disposition(WriteDisposition::Append),
        )
        .unwrap();
    {
        let mut execution = Box::pin(operation.execute());
        assert_pending(execution.as_mut());
    }
    assert_eq!(b"suffix", probe.written_bytes().as_slice());
    assert_eq!(6, operation.written_bytes());
    assert!(operation.has_recovery());
    assert_eq!(
        AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate),
        operation.state()
    );
}

#[cfg(feature = "async")]
#[test]
fn test_published_commit_error_remains_applied_after_durability_failure() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        writer_commit_error: Some(FsErrorKind::Io),
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/durability").unwrap(),
            b"data".to_vec(),
            WriteOptions::default(),
        )
        .unwrap();
    let failure = ready(operation.execute()).expect_err("post-publication error is retained");
    assert_eq!(WriteFailureState::Published, failure.state());
    assert_eq!(4, failure.written_bytes());
    assert!(operation.remaining_bytes().is_empty());
    assert_eq!(1, probe.writer_commits());
    assert_eq!(0, probe.writer_aborts());
}

#[cfg(feature = "async")]
#[test]
fn test_required_content_condition_without_guaranteed_compare_fails_before_open() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::ConditionalWrite),
        ..Default::default()
    });
    let result = filesystem.begin_write_all(
        Path::parse("/condition").unwrap(),
        b"new".to_vec(),
        WriteOptions::default()
            .with_content_precondition(ContentPrecondition::Equals(b"old".to_vec())),
    );
    assert!(result.is_err());
    assert!(!probe.calls().contains(&"open_writer"));
}

#[cfg(feature = "async")]
#[test]
fn test_pending_writer_acceptance_is_not_replayed_after_cancellation() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        pending_stage: Some(AsyncCopyStage::WriterWrite),
        writer_progress_before_stop: 2,
        writer_chunk: Some(2),
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/pending-write").unwrap(),
            b"buffer".to_vec(),
            WriteOptions::default(),
        )
        .unwrap();
    {
        let mut execution = Box::pin(operation.execute());
        assert_pending(execution.as_mut());
    }
    assert_eq!(b"bu", probe.written_bytes().as_slice());
    assert_eq!(2, operation.written_bytes());
    assert_eq!(b"ffer", operation.remaining_bytes());
    assert!(operation.has_recovery());
}

#[cfg(feature = "async")]
#[test]
fn test_cleanup_failure_keeps_temp_owner_recoverable() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        temp_cleanup_failure: true,
        ..Default::default()
    });
    let mut temporary = ready(filesystem.create_temp_file(TempOptions::default()))
        .expect("fixture creates the temporary file");
    let error = ready(temporary.cleanup()).expect_err("first cleanup failure is observable");
    assert!(error.to_string().contains("injected cleanup failure"));
    assert_eq!(TempResourceState::CleanupRequired, temporary.state());
    let retry = ready(temporary.cleanup()).expect_err("owner remains available for retry");
    assert!(retry.to_string().contains("injected cleanup failure"));
    assert_eq!(TempResourceState::CleanupRequired, temporary.state());
    assert_eq!(
        vec!["create_temp_file", "cleanup", "cleanup"],
        probe.calls()
    );
    drop(temporary);
    assert_eq!(1, probe.temp_cancellations());
}

#[cfg(feature = "async")]
#[test]
fn test_commit_failure_and_abort_failure_keep_separate_recovery_state() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        writer_abort_failure: Some(FsErrorKind::Io),
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/independent-cleanup").unwrap(),
            b"body".to_vec(),
            WriteOptions::default(),
        )
        .unwrap();
    let failure = ready(operation.execute()).expect_err("commit failure is retained");
    assert_eq!(WriteFailureState::Published, failure.state());
    let mut recovery = operation
        .take_recovery()
        .expect("writer owner remains available");
    let AsyncWriterRecovery::Opened(writer) = &mut recovery else {
        panic!("opened writer expected")
    };
    let abort = ready(writer.abort_async()).expect_err("abort failure remains independent");
    assert_eq!(FsErrorKind::Io, abort.kind());
    assert_eq!(1, probe.writer_commits());
    assert_eq!(1, probe.writer_aborts());
}

#[cfg(feature = "async")]
#[test]
fn test_temp_created_before_registration_rejection_remains_in_commit_error() {
    struct NeverCleanup;
    impl AsyncCleanup<Mutex<Option<AsyncTempFile>>> for NeverCleanup {
        fn cleanup(
            self: Arc<Self>,
            _resource: Mutex<Option<AsyncTempFile>>,
            _: CleanupContext,
        ) -> CleanupFuture<Mutex<Option<AsyncTempFile>>> {
            Box::pin(async move { CleanupOutcome::Released })
        }
    }

    let manager = OperationManager::new(ManagerOptions::default()).unwrap();
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig::default());
    let mut operation = manager
        .try_submit(OperationOptions::default(), move |scope| {
            let reservation = scope.reserve_resource().unwrap();
            let temporary = ready(filesystem.create_temp_file(TempOptions::default()))
                .expect("temporary creation succeeds before registration");
            let path = temporary.path().clone();
            let _ = scope.request_stop(StopReason::UserCancelled);
            let owner = Mutex::new(Some(temporary));
            let error = match reservation.commit_with(owner, NeverCleanup) {
                Ok(_) => panic!("stopped operation accepted resource registration"),
                Err(error) => error,
            };
            let (reason, owner, cleanup, reservation) = error.into_parts();
            assert_eq!(ResourceManageErrorKind::Stopped, reason);
            let temporary = owner
                .lock()
                .unwrap()
                .take()
                .expect("commit error retains temp owner");
            assert_eq!(path, *temporary.path());
            drop(temporary);
            drop(cleanup);
            drop(reservation);
            Ok::<(), FsError>(())
        })
        .unwrap();
    let _ = operation.wait_outcome().unwrap();
    let _ = manager
        .request_shutdown(ShutdownMode::Drain)
        .wait()
        .unwrap();
    assert!(probe.calls().contains(&"create_temp_file"));
}

#[cfg(feature = "async")]
#[test]
fn test_version_condition_without_conditional_write_fails_before_publication() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::ConditionalWrite),
        ..Default::default()
    });
    let result = filesystem.begin_write_all(
        Path::parse("/version-condition").unwrap(),
        b"new".to_vec(),
        WriteOptions::default()
            .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v1"))),
    );
    assert!(result.is_err());
    assert!(!probe.calls().contains(&"open_writer"));
}

#[cfg(feature = "async")]
#[test]
fn test_version_precondition_rejects_support_only_conditional_write_before_open() {
    struct ConditionalProvider {
        properties: ProviderProperties,
        writer_opens: Arc<AtomicUsize>,
    }
    impl AsyncFileSystemSpi for ConditionalProvider {
        fn properties(&self) -> ProviderProperties {
            self.properties.clone()
        }
        fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::NotFound,
                    FsOperation::Stat,
                    "not used",
                ))
            })
        }
        fn open_writer<'a>(
            &'a self,
            request: OpenWriterRequest<'a>,
        ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            let _ = request;
            self.writer_opens.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::OpenWriter,
                    "must be preflight rejected",
                ))
            })
        }
    }

    let properties = ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("conditional-only").unwrap(),
            "conditional-only",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new().with(ProviderOperation::OpenWriter),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Write)
            .with_conditional(FileSystemCapability::ConditionalWrite),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .unwrap();
    let writer_opens = Arc::new(AtomicUsize::new(0));
    let filesystem = AsyncFileSystem::from_spi(ConditionalProvider {
        properties,
        writer_opens: Arc::clone(&writer_opens),
    })
    .unwrap();
    let result = filesystem.begin_write_all(
        Path::parse("/version-guard").unwrap(),
        b"new".to_vec(),
        WriteOptions::default()
            .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v1"))),
    );
    let error = result.expect_err(
        "support-only conditional writes cannot guarantee atomic comparison and publication",
    );
    assert_eq!(FsErrorKind::RequirementNotMet, error.error().kind());
    assert_eq!(0, writer_opens.load(Ordering::SeqCst));
}
