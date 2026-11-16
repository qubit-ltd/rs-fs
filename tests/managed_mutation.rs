use qubit_fs::{FsEffectFact, FsEffectSlotPool, FsEffectSnapshot, FsEffectState};

#[test]
fn effect_reservation_stays_owned_until_recovery_finishes() {
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
fn prepared_mutations_accept_one_owned_effect_reservation() {
    use qubit_fs::spi::{PreparedFsCopy, PreparedFsRename, PreparedFsWrite};

    let _: fn(
        &dyn PreparedFsWrite,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsWrite::install_effect_reservation;
    let _: fn(
        &dyn PreparedFsCopy,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsCopy::install_effect_reservation;
    let _: fn(
        &dyn PreparedFsRename,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsRename::install_effect_reservation;
}

#[test]
fn a_lost_effect_is_not_reported_as_unchanged() {
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
fn failure_publication_preserves_write_count_and_publication_state() {
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use qubit_fs::write::WriteFailureState;
    use std::sync::Arc;

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
fn failure_publication_preserves_copy_counters_and_uncertainty() {
    use qubit_fs::copy::{CopyFailureState, CopyStats};
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use std::sync::Arc;

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
fn failure_publication_preserves_rename_transition_state() {
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use qubit_fs::rename::RenameFailureState;
    use std::sync::Arc;

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
fn content_and_version_conditions_are_distinct_inputs() {
    use qubit_fs::metadata::{ContentPrecondition, ResourceVersion};
    use qubit_fs::write::{WriteOptions, WritePrecondition};

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
fn write_content_condition_is_preserved_independently_from_version_condition() {
    use qubit_fs::metadata::{ContentPrecondition, ResourceVersion};
    use qubit_fs::write::{WriteOptions, WritePrecondition};

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
fn required_content_compare_needs_guaranteed_conditional_write() {
    use qubit_fs::metadata::{FileSystemCapabilities, FileSystemCapability};
    use qubit_fs::write::WriteOptions;

    let options = WriteOptions::default()
        .with_content_precondition(qubit_fs::metadata::ContentPrecondition::Equals(vec![1]));
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
#[path = "common/mod.rs"]
mod common;

#[cfg(feature = "async")]
#[test]
fn native_copy_partial_side_effect_never_opens_fallback_streams() {
    use common::poll_support::ready;
    use qubit_fs::copy::{CopyFailureState, CopyOptions, CopyStats};
    use qubit_fs::error::{FsEffectState, FsError, FsErrorKind, FsOperation};
    use qubit_fs::metadata::{
        FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo,
        FileSystemLimits, SymlinkPolicy,
    };
    use qubit_fs::path::{Path, PathConstraints, PathSemantics};
    use qubit_fs::spi::{
        AsyncFileSystemSpi, CopyAttempt, CopyRequest, OpenReaderRequest, OpenWriterRequest,
        ProviderOperation, ProviderOperations, ProviderProperties, SpiCopyFailure, SpiFuture,
        StatRequest, StatResponse,
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct SideEffectCopyProvider {
        properties: ProviderProperties,
        fallback_opens: Arc<AtomicUsize>,
    }
    impl AsyncFileSystemSpi for SideEffectCopyProvider {
        fn properties(&self) -> ProviderProperties {
            self.properties.clone()
        }

        fn stat<'a>(
            &'a self,
            _: StatRequest<'a>,
        ) -> SpiFuture<'a, qubit_fs::FsResult<StatResponse>> {
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
        ) -> SpiFuture<'a, qubit_fs::FsResult<qubit_fs::spi::OpenedAsyncReader>> {
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
        ) -> SpiFuture<'a, qubit_fs::FsResult<qubit_fs::spi::OpenedAsyncWriter>> {
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
    let filesystem = qubit_fs::AsyncFileSystem::from_spi(SideEffectCopyProvider {
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
fn append_cancellation_keeps_acknowledged_bytes_and_writer_owner() {
    use common::async_recording_spi::{
        AsyncCopyStage, AsyncRecordingConfig, async_recording_file_system,
    };
    use common::poll_support::assert_pending;
    use qubit_fs::{
        Path,
        write::{AsyncWriteAllOperationState, WriteDisposition, WriteFailureState, WriteOptions},
    };

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
fn published_commit_error_remains_applied_after_durability_failure() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use common::poll_support::ready;
    use qubit_fs::{
        Path,
        write::{WriteFailureState, WriteOptions},
    };

    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        writer_commit_error: Some(qubit_fs::error::FsErrorKind::Io),
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
fn required_content_condition_without_guaranteed_compare_fails_before_open() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use qubit_fs::{
        Path,
        metadata::{ContentPrecondition, FileSystemCapability},
        write::WriteOptions,
    };

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
fn pending_writer_acceptance_is_not_replayed_after_cancellation() {
    use common::async_recording_spi::{
        AsyncCopyStage, AsyncRecordingConfig, async_recording_file_system,
    };
    use common::poll_support::assert_pending;
    use qubit_fs::{Path, write::WriteOptions};

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
fn cleanup_failure_keeps_temp_owner_recoverable() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use common::poll_support::ready;
    use qubit_fs::temp::{TempOptions, TempResourceState};

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
fn commit_failure_and_abort_failure_keep_separate_recovery_state() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use common::poll_support::ready;
    use qubit_fs::error::FsErrorKind;
    use qubit_fs::{
        Path,
        write::{AsyncWriterRecovery, WriteFailureState, WriteOptions},
    };

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
fn temp_created_before_registration_rejection_remains_in_commit_error() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use common::poll_support::ready;
    use qubit_fs::temp::{AsyncTempFile, TempOptions};
    use qubit_operation::{
        AsyncCleanup, CleanupContext, CleanupFuture, CleanupOutcome, OperationManager,
        OperationOptions, ShutdownMode, StopReason,
    };
    use std::sync::Arc;
    use std::sync::Mutex;

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

    let manager = OperationManager::new(qubit_operation::ManagerOptions::default()).unwrap();
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
            assert_eq!(qubit_operation::ResourceManageErrorKind::Stopped, reason);
            let temporary = owner
                .lock()
                .unwrap()
                .take()
                .expect("commit error retains temp owner");
            assert_eq!(path, *temporary.path());
            drop(temporary);
            drop(cleanup);
            drop(reservation);
            Ok::<(), qubit_fs::FsError>(())
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
fn version_condition_without_conditional_write_fails_before_publication() {
    use common::async_recording_spi::{AsyncRecordingConfig, async_recording_file_system};
    use qubit_fs::{
        Path,
        metadata::{FileSystemCapability, ResourceVersion},
        write::{WriteOptions, WritePrecondition},
    };

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
fn version_precondition_rejects_support_only_conditional_write_before_open() {
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use qubit_fs::metadata::{
        FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo,
        FileSystemLimits, SymlinkPolicy,
    };
    use qubit_fs::path::{PathConstraints, PathSemantics};
    use qubit_fs::spi::{
        AsyncFileSystemSpi, OpenWriterRequest, ProviderOperation, ProviderOperations,
        ProviderProperties, SpiFuture, StatRequest, StatResponse,
    };
    use qubit_fs::{
        AsyncFileSystem, Path,
        metadata::ResourceVersion,
        write::{WriteOptions, WritePrecondition},
    };
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct ConditionalProvider {
        properties: ProviderProperties,
        writer_opens: Arc<AtomicUsize>,
    }
    impl AsyncFileSystemSpi for ConditionalProvider {
        fn properties(&self) -> ProviderProperties {
            self.properties.clone()
        }
        fn stat<'a>(
            &'a self,
            _: StatRequest<'a>,
        ) -> SpiFuture<'a, qubit_fs::FsResult<StatResponse>> {
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
        ) -> SpiFuture<'a, qubit_fs::FsResult<qubit_fs::spi::OpenedAsyncWriter>> {
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
