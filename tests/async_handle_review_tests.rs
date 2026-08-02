// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Regression coverage for asynchronous facade-handle contract boundaries.

#![cfg(feature = "async")]

#[allow(dead_code)]
#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[allow(dead_code)]
#[path = "common/poll_support.rs"]
mod poll_support;

use std::io::Result as IoResult;
use std::pin::Pin;
use std::task::{
    Context,
    Poll,
};

use qubit_fs::spi::{
    AsyncFileWriteSession,
    SpiFuture,
};
use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    DirEntry,
    FileKind,
    FsErrorKind,
    ListOptions,
    Path,
    PersistOptions,
    TempDirectoryOptions,
    TempFileOptions,
    WriteFailureState,
    WriteOptions,
    WriterState,
};
use qubit_io::{
    AsyncInput,
    AsyncOutput,
};

use crate::async_recording_spi::{
    AsyncCopyStage,
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::ready;

/// Parses an absolute path used by one regression scenario.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Minimal provider session that deliberately relies on the trait's no-op
/// cancellation default.
struct DefaultCancelSession;

impl AsyncOutput for DefaultCancelSession {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &[u8],
        _: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(count))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for DefaultCancelSession {
    fn commit_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<qubit_fs::WriteOutcome, qubit_fs::WriteFailure>>
    {
        panic!("default cancellation test does not commit")
    }

    fn abort_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, qubit_fs::FsResult<qubit_fs::WriteAbortOutcome>> {
        panic!("default cancellation test does not abort")
    }
}

/// Confirms the default drop-cancellation hook is a nonblocking no-op.
#[test]
fn test_async_file_write_session_default_cancel_on_drop_is_noop() {
    let mut session = DefaultCancelSession;
    Pin::new(&mut session).cancel_on_drop();
}

/// Rejects paths and provider identities that do not match the facade request.
#[test]
fn test_async_facade_rejects_invalid_stat_and_opened_identities() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_stat_path: true,
        ..AsyncRecordingConfig::default()
    });
    let error = ready(file_system.stat(&path("/expected")))
        .expect_err("a stat response for another path must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_opened_identity: true,
        ..AsyncRecordingConfig::default()
    });
    let reader = ready(
        file_system
            .open_reader(&path("/expected"), qubit_fs::ReadOptions::default()),
    )
    .expect_err("a reader from another provider must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, reader.kind());
    let writer = ready(
        file_system.open_writer(&path("/expected"), WriteOptions::default()),
    )
    .expect_err("a writer from another provider must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, writer.kind());
}

/// Enriches provider failures consistently across every direct facade
/// operation, without treating those failures as contract violations.
#[test]
fn test_async_facade_enriches_direct_provider_failures() {
    let target = path("/target");
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        list_open_error: true,
        create_directory_error: true,
        delete_error: true,
        rename_error: true,
        ..AsyncRecordingConfig::default()
    });

    for error in [
        ready(file_system.list(&target, ListOptions::default()))
            .expect_err("list provider failure should propagate"),
        ready(file_system.create_directory(
            &target,
            qubit_fs::CreateDirectoryOptions::default(),
        ))
        .expect_err("create provider failure should propagate"),
        ready(
            file_system
                .delete_file(&target, qubit_fs::DeleteOptions::default()),
        )
        .expect_err("delete-file provider failure should propagate"),
        ready(
            file_system
                .delete_directory(&target, qubit_fs::DeleteOptions::default()),
        )
        .expect_err("delete-directory provider failure should propagate"),
    ] {
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some(&target), error.path());
        assert_eq!(Some("async-recording"), error.provider());
    }
    let rename = ready(file_system.rename(
        &path("/source"),
        &target,
        qubit_fs::RenameOptions::default(),
    ))
    .expect_err("rename provider failure should propagate");
    assert_eq!(FsErrorKind::UnsupportedOperation, rename.error().kind());
    assert_eq!(qubit_fs::RenameFailureState::Indeterminate, rename.state());
}

/// Propagates direct handle and temporary-resource open failures with the
/// facade's requested operation context.
#[test]
fn test_async_facade_enriches_handle_and_temp_provider_failures() {
    let target = path("/target");
    for stage in [AsyncCopyStage::OpenReader, AsyncCopyStage::OpenWriter] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                failing_stage: Some(stage),
                ..AsyncRecordingConfig::default()
            });
        let error = match stage {
            AsyncCopyStage::OpenReader => ready(
                file_system
                    .open_reader(&target, qubit_fs::ReadOptions::default()),
            )
            .expect_err("reader provider failure should propagate"),
            AsyncCopyStage::OpenWriter => {
                ready(file_system.open_writer(&target, WriteOptions::default()))
                    .expect_err("writer provider failure should propagate")
            }
            _ => unreachable!("only handle stages are configured"),
        };
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some(&target), error.path());
    }
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        temp_create_error: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(file) =
        ready(file_system.create_temp_file(TempFileOptions::default()))
    else {
        panic!("temporary-file provider failure should propagate");
    };
    assert_eq!(FsErrorKind::UnsupportedOperation, file.kind());
    let Err(directory) = ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    ) else {
        panic!("temporary-directory provider failure should propagate");
    };
    assert_eq!(FsErrorKind::UnsupportedOperation, directory.kind());
}

/// Rejects a non-atomic provider writer outcome when the caller requires atomic
/// publication.
#[test]
fn test_async_writer_rechecks_required_atomic_commit_outcome() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_atomicity: Some(AchievedAtomicity::NonAtomic),
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut writer = ready(file_system.open_writer(
        &path("/final"),
        WriteOptions::default().with_atomicity(AtomicityRequirement::Required),
    ))
    .expect("provider advertises atomic write support");
    let error = ready(writer.commit_async()).expect_err(
        "a non-atomic success must not satisfy a required atomic write",
    );
    assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
    assert_eq!(WriterState::Published, writer.state());
}

/// Retains publication state when abort cleanup confirms success after
/// publication.
#[test]
fn test_async_writer_abort_preserves_published_state() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        ..AsyncRecordingConfig::default()
    });
    let mut writer = ready(
        file_system.open_writer(&path("/final"), WriteOptions::default()),
    )
    .expect("writer should open");
    ready(writer.commit_async())
        .expect_err("provider should report published failure");
    let _ = ready(writer.abort_async()).expect("cleanup should succeed");
    assert_eq!(WriterState::Published, writer.state());
}

/// Rejects a non-atomic successful temporary persist and retains source cleanup
/// responsibility.
#[test]
fn test_async_temp_persist_rechecks_required_atomic_outcome() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        temp_persist_atomicity: Some(AchievedAtomicity::NonAtomic),
        ..AsyncRecordingConfig::default()
    });
    let mut temp =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    let failure = ready(
        temp.persist(
            &path("/final"),
            PersistOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        ),
    )
    .expect_err("a non-atomic persist must fail the required atomic contract");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(
        qubit_fs::PersistFailureState::PublishedSourceRetained,
        failure.state()
    );
    assert_eq!(qubit_fs::TempResourceState::CleanupRequired, temp.state());
}

/// Rejects entries outside the list root and transitions the stream to a
/// terminal state.
#[test]
fn test_async_directory_stream_rejects_outside_root_and_becomes_terminal() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(
            path("/outside"),
            FileKind::File,
        )],
        ..AsyncRecordingConfig::default()
    });
    let mut stream =
        ready(file_system.list(&path("/root"), ListOptions::default()))
            .expect("directory stream should open");
    let error = ready(stream.next_entry_async())
        .expect_err("outside entry must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    let terminal = ready(stream.next_entry_async())
        .expect_err("invalid stream must be terminal");
    assert_eq!(FsErrorKind::InvalidState, terminal.kind());
}

/// Verifies a nested prefix is accepted without enabling recursive listing.
#[test]
fn test_async_directory_stream_accepts_nested_prefix_without_recursive_option()
{
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(
            path("/root/nested/item"),
            FileKind::File,
        )],
        ..AsyncRecordingConfig::default()
    });
    let mut stream = ready(file_system.list(
        &path("/root"),
        ListOptions::default().with_prefix(Some("nested/item".to_owned())),
    ))
    .expect("directory stream should open");

    let entry = ready(stream.next_entry_async())
        .expect("nested prefix must be accepted")
        .expect("matching entry must be returned");
    assert_eq!(path("/root/nested/item"), entry.path);
}

/// Treats a provider enumeration failure as terminal for all later reads.
#[test]
fn test_async_directory_stream_error_becomes_terminal() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_error: true,
        ..AsyncRecordingConfig::default()
    });
    let mut stream =
        ready(file_system.list(&path("/root"), ListOptions::default()))
            .expect("directory stream should open");
    let error = ready(stream.next_entry_async())
        .expect_err("provider failure should propagate");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    let terminal = ready(stream.next_entry_async())
        .expect_err("failed stream must be terminal");
    assert_eq!(FsErrorKind::InvalidState, terminal.kind());
}

/// Rejects a nested entry from a non-recursive request unless the request
/// explicitly names the nested lexical prefix.
#[test]
fn test_async_directory_stream_rejects_nested_entry_for_direct_listing() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(
            path("/root/nested/item"),
            FileKind::File,
        )],
        ..AsyncRecordingConfig::default()
    });
    let mut stream =
        ready(file_system.list(&path("/root"), ListOptions::default()))
            .expect("directory stream should open");

    let error = ready(stream.next_entry_async()).expect_err(
        "non-recursive listing must reject nested provider entries",
    );
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert!(format!("{stream:?}").contains("AsyncDirectoryStream"));
}

/// Rejects an entry that omits metadata after the caller requested it.
#[test]
fn test_async_directory_stream_rejects_missing_requested_metadata() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(
            path("/root/file"),
            FileKind::File,
        )],
        ..AsyncRecordingConfig::default()
    });
    let mut stream = ready(file_system.list(
        &path("/root"),
        ListOptions::default().with_include_metadata(true),
    ))
    .expect("directory stream should open");

    let error = ready(stream.next_entry_async()).expect_err(
        "metadata request must be enforced against provider entries",
    );
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

/// Rejects asynchronous provider entries whose identity fields disagree.
#[test]
fn test_async_directory_stream_rejects_inconsistent_entry_identity() {
    let mut entry = DirEntry::new(path("/root/file"), FileKind::File);
    entry.name = "other".to_owned();
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![entry],
        ..AsyncRecordingConfig::default()
    });
    let mut stream =
        ready(file_system.list(&path("/root"), ListOptions::default()))
            .expect("directory stream should open");
    let error = ready(stream.next_entry_async())
        .expect_err("inconsistent entry name must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

/// Accepts descendants of an explicit prefix and evaluates root-relative
/// entries without treating the root itself as a required child prefix.
#[test]
fn test_async_directory_stream_accepts_prefix_descendant_and_root_entry() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(
            path("/root/nested/item"),
            FileKind::File,
        )],
        ..AsyncRecordingConfig::default()
    });
    let mut stream = ready(file_system.list(
        &path("/root"),
        ListOptions::default().with_prefix(Some("nested".to_owned())),
    ))
    .expect("directory stream should open");
    assert!(
        ready(stream.next_entry_async())
            .expect("prefix descendant should be accepted")
            .is_some()
    );

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        directory_entries: vec![DirEntry::new(path("/file"), FileKind::File)],
        ..AsyncRecordingConfig::default()
    });
    let mut root =
        ready(file_system.list(&Path::root(), ListOptions::default()))
            .expect("root stream should open");
    assert!(
        ready(root.next_entry_async())
            .expect("root-relative entry should be accepted")
            .is_some()
    );
}

/// Exercises the successful asynchronous facade paths with a provider that
/// records each dispatched primitive.
#[test]
fn test_async_facade_dispatches_successful_operations() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            rename_atomicity: Some(AchievedAtomicity::Atomic),
            ..AsyncRecordingConfig::default()
        });
    let source = path("/source");
    let target = path("/target");

    assert_eq!(
        Some(5),
        ready(file_system.stat(&source))
            .expect("stat should succeed")
            .len()
    );
    let mut stream = ready(file_system.list(&source, ListOptions::default()))
        .expect("list should succeed");
    assert!(
        ready(stream.next_entry_async())
            .expect("empty stream should succeed")
            .is_none()
    );

    let mut reader = ready(
        file_system.open_reader(&source, qubit_fs::ReadOptions::default()),
    )
    .expect("reader should open");
    assert_eq!(&source, reader.info().path());
    assert!(!reader.is_buffered());
    assert!(format!("{reader:?}").contains("AsyncFileReader"));
    let mut bytes = [0; 5];
    assert_eq!(
        5,
        ready(reader.read_fully_async(&mut bytes))
            .expect("reader should return bytes")
    );
    assert_eq!(b"bytes", &bytes);

    let mut writer =
        ready(file_system.open_writer(&target, WriteOptions::default()))
            .expect("writer should open");
    assert_eq!(&target, writer.info().path());
    assert_eq!(WriterState::Open, writer.state());
    assert!(!writer.is_buffered());
    assert!(format!("{writer:?}").contains("AsyncFileWriter"));
    ready(writer.write_fully_async(b"bytes"))
        .expect("writer should accept all bytes");
    ready(writer.flush_async()).expect("writer should flush");
    ready(writer.commit_async()).expect("writer should commit");
    assert_eq!(WriterState::Committed, writer.state());

    assert!(
        !ready(file_system.create_directory(
            &target,
            qubit_fs::CreateDirectoryOptions::default(),
        ))
        .expect("directory creation should succeed")
        .already_existed()
    );
    assert!(
        !ready(
            file_system
                .delete_file(&target, qubit_fs::DeleteOptions::default(),)
        )
        .expect("file deletion should succeed")
        .already_missing()
    );
    assert!(
        !ready(
            file_system
                .delete_directory(&target, qubit_fs::DeleteOptions::default(),)
        )
        .expect("directory deletion should succeed")
        .already_missing()
    );
    assert_eq!((source.clone(), target.clone()), {
        let outcome = ready(file_system.rename(
            &source,
            &target,
            qubit_fs::RenameOptions::default(),
        ))
        .expect("rename should succeed");
        (outcome.source().clone(), outcome.target().clone())
    });
    assert_eq!(
        vec![
            "stat",
            "open_reader",
            "open_writer",
            "create_directory",
            "delete_file",
            "delete_directory",
            "rename",
        ],
        probe.calls()
    );
}

/// Marks an asynchronous writer indeterminate when either byte transfer or
/// flushing fails, preventing an unsafe later commit.
#[test]
fn test_async_writer_stream_failures_mark_writer_indeterminate() {
    for stage in [AsyncCopyStage::WriterWrite, AsyncCopyStage::WriterFlush] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                failing_stage: Some(stage),
                ..AsyncRecordingConfig::default()
            });
        let mut writer = ready(
            file_system.open_writer(&path("/target"), WriteOptions::default()),
        )
        .expect("writer should open");
        let error = if stage == AsyncCopyStage::WriterWrite {
            ready(writer.write_fully_async(b"bytes"))
                .expect_err("injected write failure should propagate")
        } else {
            ready(writer.flush_async())
                .expect_err("injected flush failure should propagate")
        };
        assert!(error.to_string().contains("injected"));
        assert_eq!(WriterState::Indeterminate, writer.state());
        let commit = ready(writer.commit_async())
            .expect_err("indeterminate writer must not commit again");
        assert_eq!(FsErrorKind::InvalidState, commit.error().kind());
    }
}

/// Preserves every provider-confirmed commit state and applies the explicit
/// abort publication outcome exactly once.
#[test]
fn test_async_writer_commit_failures_preserve_recovery_state() {
    for (failure_state, expected_state, abort_outcome, aborted_state) in [
        (
            WriteFailureState::RetryableNotPublished,
            WriterState::Open,
            qubit_fs::WriteAbortOutcome::NotPublished,
            WriterState::Aborted,
        ),
        (
            WriteFailureState::NotPublished,
            WriterState::NotPublished,
            qubit_fs::WriteAbortOutcome::NotPublished,
            WriterState::Aborted,
        ),
        (
            WriteFailureState::Published,
            WriterState::Published,
            qubit_fs::WriteAbortOutcome::Published,
            WriterState::Published,
        ),
        (
            WriteFailureState::Indeterminate,
            WriterState::Indeterminate,
            qubit_fs::WriteAbortOutcome::Indeterminate,
            WriterState::Indeterminate,
        ),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                writer_commit_failure: Some(failure_state),
                ..AsyncRecordingConfig::default()
            });
        let mut writer = ready(
            file_system.open_writer(&path("/target"), WriteOptions::default()),
        )
        .expect("writer should open");
        let error = ready(writer.commit_async())
            .expect_err("configured commit failure should propagate");
        assert_eq!(FsErrorKind::Io, error.error().kind());
        assert_eq!(expected_state, writer.state());
        assert_eq!(
            abort_outcome,
            ready(writer.abort_async()).expect("failed writer should abort"),
        );
        assert_eq!(aborted_state, writer.state());
        let abort = ready(writer.abort_async())
            .expect_err("completed abort must reject a second abort");
        assert_eq!(FsErrorKind::InvalidState, abort.kind());
    }
}

/// Restores the original state after a definite abort failure while retaining
/// an indeterminate abort failure as an indeterminate session.
#[test]
fn test_async_writer_abort_failure_tracks_certainty() {
    for (kind, expected_state) in [
        (FsErrorKind::Io, WriterState::Open),
        (FsErrorKind::Indeterminate, WriterState::Indeterminate),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                writer_abort_failure: Some(kind),
                ..AsyncRecordingConfig::default()
            });
        let mut writer = ready(
            file_system.open_writer(&path("/target"), WriteOptions::default()),
        )
        .expect("writer should open");
        let error = ready(writer.abort_async())
            .expect_err("configured abort failure should propagate");
        assert_eq!(kind, error.kind());
        assert_eq!(expected_state, writer.state());
    }
}

/// Enforces the provider-advertised cumulative byte limit and prevents byte
/// transfer after a writer has completed publication.
#[test]
fn test_async_writer_enforces_limit_and_closed_state() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        maximum_write_bytes: Some(3),
        ..AsyncRecordingConfig::default()
    });
    let mut writer = ready(
        file_system.open_writer(&path("/target"), WriteOptions::default()),
    )
    .expect("writer should open");
    let limit = ready(writer.write_fully_async(b"four"))
        .expect_err("finite write limit should reject oversized transfer");
    assert!(limit.to_string().contains("provider byte limit"));

    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut writer = ready(
        file_system.open_writer(&path("/target"), WriteOptions::default()),
    )
    .expect("writer should open");
    ready(writer.commit_async()).expect("writer should commit");
    let closed = ready(writer.write_fully_async(b"bytes"))
        .expect_err("committed writer must reject byte transfer");
    assert!(
        closed
            .to_string()
            .contains("writer no longer accepts bytes")
    );
    let flush = ready(writer.flush_async())
        .expect_err("committed writer must reject flushing");
    assert!(flush.to_string().contains("writer no longer accepts bytes"));
}

/// Rejects provider outcomes that claim an accepted pre-existing or missing
/// resource without the caller opting into that behavior.
#[test]
fn test_async_facade_rejects_unrequested_idempotent_outcomes() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        create_directory_already_existed: true,
        delete_already_missing: true,
        ..AsyncRecordingConfig::default()
    });
    let target = path("/target");

    let create_error = ready(file_system.create_directory(
        &target,
        qubit_fs::CreateDirectoryOptions::default(),
    ))
    .expect_err("existing directory without exists_ok must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, create_error.kind());
    let delete_error = ready(
        file_system.delete_file(&target, qubit_fs::DeleteOptions::default()),
    )
    .expect_err("missing file without missing_ok must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, delete_error.kind());
    let directory_error = ready(
        file_system
            .delete_directory(&target, qubit_fs::DeleteOptions::default()),
    )
    .expect_err("missing directory without missing_ok must fail");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        directory_error.kind()
    );
}

/// Uses the public copy operation to exercise the asynchronous stream
/// fallback when the provider explicitly declines its native primitive.
#[test]
fn test_async_copy_stream_fallback_reads_writes_and_commits() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            qubit_fs::CopyOptions::default(),
        )
        .expect("copy preflight should succeed");

    let outcome = ready(operation.execute())
        .expect("declined native copy should stream through facade handles");
    assert_eq!(5, outcome.stats().bytes);
    assert_eq!(1, outcome.stats().files);
    assert_eq!(&path("/source"), operation.source());
    assert_eq!(&path("/target"), operation.target());
    assert_eq!(
        qubit_fs::AsyncCopyOperationState::Completed,
        operation.state()
    );
    assert!(!operation.has_recovery_writer());
    assert!(operation.take_recovery_writer().is_none());
    let retry = ready(operation.execute())
        .expect_err("completed copy operation must reject a second execute");
    assert_eq!(FsErrorKind::InvalidState, retry.error().kind());
    assert_eq!(
        vec!["try_copy", "stat", "open_reader", "open_writer"],
        probe.calls()
    );
}

/// Exposes a retained writer after a partially published copy failure so a
/// caller can make an explicit recovery decision.
#[test]
fn test_async_copy_failure_exposes_recovery_writer_accessor() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        failing_stage: Some(AsyncCopyStage::WriterWrite),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            qubit_fs::CopyOptions::default(),
        )
        .expect("copy preflight should succeed");
    ready(operation.execute())
        .expect_err("configured transfer failure should propagate");
    assert!(operation.recovery_writer().is_some());
    assert!(operation.take_recovery_writer().is_some());
    assert!(!operation.has_recovery_writer());
}
