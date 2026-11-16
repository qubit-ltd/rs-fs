// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Regression coverage for independent writer publication and cleanup facts.

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::RecoveryCleanupState;
use qubit_fs::write::FileWriter;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterState;

mod handle_support;

fn opened_writer(commit_failure: Option<WriteFailureState>, abort_failure: Option<FsErrorKind>) -> FileWriter {
    handle_support::implementation_tests::writer_lifecycle_filesystem(commit_failure, abort_failure)
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open")
}

#[test]
fn test_sync_writer_keeps_publication_history_after_cleanup_failure() {
    let (filesystem, commit_calls, _) = handle_support::implementation_tests::writer_lifecycle_filesystem_with_counts(
        Some(WriteFailureState::Published),
        Some(FsErrorKind::Io),
    );
    let mut writer = filesystem
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open");
    assert!(!writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Pending, writer.cleanup_state());

    let failure = writer.commit().expect_err("configured commit should fail");
    assert_eq!(WriteFailureState::Published, failure.state());
    assert!(writer.has_confirmed_publication());

    let cleanup = writer.abort().expect_err("configured cleanup should fail");
    assert_eq!(FsErrorKind::Io, cleanup.kind());
    assert!(writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Indeterminate, writer.cleanup_state());

    let repeated = writer
        .commit()
        .expect_err("published writer must reject a second commit");
    assert_eq!(FsErrorKind::InvalidState, repeated.error().kind());
    assert_eq!(WriteFailureState::Published, repeated.state());
    assert_eq!(1, *commit_calls.lock().expect("counter lock should succeed"));
}

#[test]
fn test_sync_writer_abort_published_confirms_history_and_cleanup() {
    let mut writer = opened_writer(Some(WriteFailureState::Published), None);
    assert!(!writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Pending, writer.cleanup_state());

    assert_eq!(
        WriteFailureState::Published,
        writer.commit().expect_err("commit should fail").state()
    );
    assert_eq!(
        WriteAbortOutcome::Published,
        writer.abort().expect("abort should complete")
    );
    assert!(writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Completed, writer.cleanup_state());
    assert_eq!(WriterState::Published, writer.state());
}

#[test]
fn test_sync_writer_successful_commit_confirms_publication() {
    let mut writer = opened_writer(None, None);
    assert!(!writer.has_confirmed_publication());

    writer.commit().expect("commit should succeed");

    assert!(writer.has_confirmed_publication());
}

#[test]
fn test_sync_writer_repeated_abort_does_not_call_provider_again() {
    let (filesystem, _, abort_calls) =
        handle_support::implementation_tests::writer_lifecycle_filesystem_with_counts(None, None);
    let mut writer = filesystem
        .open_writer(
            &Path::parse("/target").expect("path should parse"),
            WriteOptions::default(),
        )
        .expect("writer should open");

    assert_eq!(
        WriteAbortOutcome::NotPublished,
        writer.abort().expect("abort should succeed")
    );
    assert_eq!(RecoveryCleanupState::Completed, writer.cleanup_state());
    let repeated = writer.abort().expect_err("completed cleanup must reject another abort");

    assert_eq!(FsErrorKind::InvalidState, repeated.kind());
    assert_eq!(1, *abort_calls.lock().expect("counter lock should succeed"));
}

#[test]
fn test_sync_writer_indeterminate_abort_completes_cleanup_without_publication_fact() {
    let mut writer = opened_writer(Some(WriteFailureState::Indeterminate), None);
    let failure = writer.commit().expect_err("configured commit should fail ambiguously");
    assert_eq!(WriteFailureState::Indeterminate, failure.state());
    assert!(!writer.has_confirmed_publication());

    assert_eq!(
        WriteAbortOutcome::Indeterminate,
        writer
            .abort()
            .expect("successful cleanup may report unknown publication")
    );
    assert!(!writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Completed, writer.cleanup_state());
}

#[cfg(feature = "async")]
#[test]
fn test_async_writer_published_commit_survives_successful_abort() {
    use common::async_recording_spi::AsyncRecordingConfig;
    use common::async_recording_spi::async_recording_file_system;
    use common::poll_support::ready;

    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        ..AsyncRecordingConfig::default()
    });
    let mut writer = ready(filesystem.open_writer(
        &Path::parse("/target").expect("path should parse"),
        WriteOptions::default(),
    ))
    .expect("writer should open");
    assert!(!writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Pending, writer.cleanup_state());

    let failure = ready(writer.commit_async()).expect_err("configured commit should fail");
    assert_eq!(WriteFailureState::Published, failure.state());
    assert!(writer.has_confirmed_publication());

    let _ = ready(writer.abort_async()).expect("abort should complete");
    assert!(writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Completed, writer.cleanup_state());
    let repeated = ready(writer.commit_async()).expect_err("published writer must reject a second commit");
    assert_eq!(WriteFailureState::Published, repeated.state());
    assert_eq!(1, probe.writer_commits());
}

#[cfg(feature = "async")]
#[test]
fn test_async_writer_successful_commit_confirms_publication() {
    use common::async_recording_spi::AsyncRecordingConfig;
    use common::async_recording_spi::async_recording_file_system;
    use common::poll_support::ready;

    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig::default());
    let mut writer = ready(filesystem.open_writer(
        &Path::parse("/target").expect("path should parse"),
        WriteOptions::default(),
    ))
    .expect("writer should open");
    assert!(!writer.has_confirmed_publication());

    ready(writer.commit_async()).expect("commit should succeed");

    assert!(writer.has_confirmed_publication());
}

#[cfg(feature = "async")]
#[test]
fn test_async_unpolled_abort_future_does_not_change_writer_or_call_provider() {
    use common::async_recording_spi::AsyncRecordingConfig;
    use common::async_recording_spi::async_recording_file_system;
    use common::poll_support::ready;

    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig::default());
    let mut writer = ready(filesystem.open_writer(
        &Path::parse("/target").expect("path should parse"),
        WriteOptions::default(),
    ))
    .expect("writer should open");
    let abort = writer.abort_async();
    drop(abort);

    assert!(!writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Pending, writer.cleanup_state());
    assert_eq!(WriterState::Open, writer.state());
    assert_eq!(0, probe.writer_aborts());
}

#[cfg(feature = "async")]
#[test]
fn test_async_writer_repeated_abort_does_not_call_provider_again() {
    use common::async_recording_spi::AsyncRecordingConfig;
    use common::async_recording_spi::async_recording_file_system;
    use common::poll_support::ready;

    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig::default());
    let mut writer = ready(filesystem.open_writer(
        &Path::parse("/target").expect("path should parse"),
        WriteOptions::default(),
    ))
    .expect("writer should open");

    assert_eq!(
        WriteAbortOutcome::NotPublished,
        ready(writer.abort_async()).expect("abort should succeed")
    );
    assert_eq!(RecoveryCleanupState::Completed, writer.cleanup_state());
    let repeated = ready(writer.abort_async()).expect_err("completed cleanup must reject another abort");

    assert_eq!(FsErrorKind::InvalidState, repeated.kind());
    assert_eq!(1, probe.writer_aborts());
}

#[cfg(feature = "async")]
#[test]
fn test_async_writer_abort_cancellation_keeps_publication_history() {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use common::async_recording_spi::AsyncCopyStage;
    use common::async_recording_spi::AsyncRecordingConfig;
    use common::async_recording_spi::async_recording_file_system;
    use common::poll_support::ready;

    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        pending_stage: Some(AsyncCopyStage::WriterAbort),
        ..AsyncRecordingConfig::default()
    });
    let mut writer = ready(filesystem.open_writer(
        &Path::parse("/target").expect("path should parse"),
        WriteOptions::default(),
    ))
    .expect("writer should open");
    let _ = ready(writer.commit_async()).expect_err("configured commit should fail");
    assert!(writer.has_confirmed_publication());

    let mut abort = writer.abort_async();
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    assert!(matches!(Pin::new(&mut abort).poll(&mut context), Poll::Pending));
    drop(abort);

    assert!(writer.has_confirmed_publication());
    assert_eq!(RecoveryCleanupState::Indeterminate, writer.cleanup_state());
    assert_eq!(WriterState::Indeterminate, writer.state());
}

#[cfg(feature = "async")]
mod common;
