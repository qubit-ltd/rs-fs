// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Owning write publication facts observed through the public facade.

use qubit_fs::Path;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsErrorKind;
use qubit_fs::write::AsyncWriteAllOperationState;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;

use crate::async_recording_spi::AsyncCopyStage;
use crate::async_recording_spi::AsyncRecordingConfig;
use crate::async_recording_spi::async_recording_file_system;
use crate::poll_support::assert_pending;
use crate::poll_support::ready;

#[test]
fn test_open_failure_requires_explicit_unchanged_evidence() {
    for (kind, effect, expected) in [
        (
            FsErrorKind::Io,
            Some(FsEffectState::Unchanged),
            WriteFailureState::NotPublished,
        ),
        (FsErrorKind::Io, None, WriteFailureState::Indeterminate),
        (
            FsErrorKind::Io,
            Some(FsEffectState::Applied),
            WriteFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Io,
            Some(FsEffectState::PartiallyApplied),
            WriteFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Io,
            Some(FsEffectState::Indeterminate),
            WriteFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Indeterminate,
            Some(FsEffectState::Unchanged),
            WriteFailureState::Indeterminate,
        ),
    ] {
        let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
            writer_open_error: Some(kind),
            writer_open_effect: effect,
            ..Default::default()
        });
        let mut operation = filesystem
            .begin_write_all(
                Path::parse("/target").expect("valid target"),
                b"bytes".to_vec(),
                WriteOptions::default(),
            )
            .expect("preflight succeeds");
        let failure = ready(operation.execute()).expect_err("provider open fails");
        assert_eq!(expected, failure.state(), "kind={kind:?}, effect={effect:?}");
        assert_eq!(effect, failure.error().effect_state());
        assert!(!operation.has_recovery_writer());
        assert_eq!(vec!["open_writer"], probe.calls());
    }
}

#[test]
fn test_success_snapshots_bytes_and_releases_completed_writer() {
    let (filesystem, _) = async_recording_file_system(Default::default());
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/target").expect("valid target"),
            b"bytes".to_vec(),
            WriteOptions::default(),
        )
        .expect("preflight succeeds");
    ready(operation.execute()).expect("write succeeds");
    assert_eq!(5, operation.written_bytes(), "success retains confirmed byte count");
    assert!(
        !operation.has_recovery_writer(),
        "committed writer has no recovery responsibility"
    );
}

#[test]
fn test_operation_owns_request_and_repeated_success_preserves_snapshot() {
    let (mut operation, probe) = {
        let (filesystem, probe) = async_recording_file_system(Default::default());
        let bytes = b"owned".to_vec();
        (
            filesystem
                .begin_write_all(Path::parse("/owned").unwrap(), bytes, WriteOptions::default())
                .unwrap(),
            probe,
        )
    };
    assert_eq!("/owned", operation.path().to_string());
    assert_eq!(
        "async-recording",
        operation.filesystem().properties().info().provider_id()
    );
    ready(operation.execute()).unwrap();
    let calls = probe.calls();
    let failure = ready(operation.execute()).unwrap_err();
    assert_eq!(FsErrorKind::InvalidState, failure.error().kind());
    assert_eq!(WriteFailureState::Published, failure.state());
    assert_eq!(5, failure.written_bytes());
    assert_eq!(AsyncWriteAllOperationState::Completed, operation.state());
    assert_eq!(calls, probe.calls());
    assert_eq!(0, probe.writer_cancellations());
}

#[test]
fn test_unpolled_execute_preserves_ready_request() {
    let (filesystem, probe) = async_recording_file_system(Default::default());
    let mut operation = filesystem
        .begin_write_all(Path::parse("/empty").unwrap(), Vec::new(), WriteOptions::default())
        .unwrap();
    drop(operation.execute());
    assert_eq!(AsyncWriteAllOperationState::Ready, operation.state());
    assert!(probe.calls().is_empty());
    ready(operation.execute()).unwrap();
    assert_eq!(0, operation.written_bytes());
    assert!(!operation.has_recovery_writer());
}

#[test]
fn test_cancellation_preserves_confirmed_progress_and_recovery_ownership() {
    for (stage, expected_bytes, has_writer) in [
        (AsyncCopyStage::OpenWriter, 0, false),
        (AsyncCopyStage::WriterWrite, 0, true),
        (AsyncCopyStage::WriterFlush, 5, true),
        (AsyncCopyStage::WriterCommit, 5, true),
    ] {
        let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
            pending_stage: Some(stage),
            ..Default::default()
        });
        let mut operation = filesystem
            .begin_write_all(
                Path::parse("/cancel").unwrap(),
                b"bytes".to_vec(),
                WriteOptions::default(),
            )
            .unwrap();
        {
            let mut execution = Box::pin(operation.execute());
            assert_pending(execution.as_mut());
        }
        let state = AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate);
        assert_eq!(state, operation.state(), "{stage:?}");
        assert_eq!(expected_bytes, operation.written_bytes(), "{stage:?}");
        assert_eq!(has_writer, operation.has_recovery_writer());
        let calls = probe.calls();
        let failure = ready(operation.execute()).unwrap_err();
        assert_eq!(FsErrorKind::InvalidState, failure.error().kind());
        assert_eq!(expected_bytes, failure.written_bytes());
        assert_eq!(calls, probe.calls());
        let writer = operation.take_recovery_writer();
        assert_eq!(has_writer, writer.is_some());
        assert!(!operation.has_recovery_writer());
        assert_eq!(state, operation.state());
        assert_eq!(expected_bytes, operation.written_bytes());
    }
}

#[test]
fn test_short_writes_count_all_confirmed_bytes() {
    for chunk in [1, 2, 4] {
        let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
            writer_chunk: Some(chunk),
            ..Default::default()
        });
        let mut operation = filesystem
            .begin_write_all(
                Path::parse("/short").unwrap(),
                b"bytes".to_vec(),
                WriteOptions::default(),
            )
            .unwrap();
        ready(operation.execute()).unwrap();
        assert_eq!(5, operation.written_bytes());
        assert!(!operation.has_recovery_writer());
    }
}

#[test]
fn test_partial_write_failure_and_cancellation_count_only_acknowledged_bytes() {
    for pending in [false, true] {
        let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
            writer_chunk: Some(1),
            writer_progress_before_stop: 3,
            pending_stage: pending.then_some(AsyncCopyStage::WriterWrite),
            failing_stage: (!pending).then_some(AsyncCopyStage::WriterWrite),
            ..Default::default()
        });
        let mut operation = filesystem
            .begin_write_all(
                Path::parse("/partial").unwrap(),
                b"bytes".to_vec(),
                WriteOptions::default(),
            )
            .unwrap();
        if pending {
            let mut execution = Box::pin(operation.execute());
            assert_pending(execution.as_mut());
        } else {
            let failure = ready(operation.execute()).unwrap_err();
            assert_eq!(3, failure.written_bytes());
            assert_eq!(WriteFailureState::Indeterminate, failure.state());
        }
        assert_eq!(3, operation.written_bytes());
        assert!(operation.has_recovery_writer());
        let snapshot = operation.state();
        let mut writer = operation.take_recovery_writer().unwrap();
        let _outcome = ready(writer.abort_async()).unwrap();
        assert_eq!(snapshot, operation.state());
        assert_eq!(3, operation.written_bytes());
    }
}

#[test]
fn test_flush_and_commit_failures_preserve_progress_and_abort_failure_retains_writer() {
    for state in [
        WriteFailureState::NotPublished,
        WriteFailureState::Published,
        WriteFailureState::Indeterminate,
    ] {
        let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
            writer_commit_failure: Some(state),
            writer_abort_failure: Some(FsErrorKind::Io),
            ..Default::default()
        });
        let mut operation = filesystem
            .begin_write_all(
                Path::parse("/commit").unwrap(),
                b"bytes".to_vec(),
                WriteOptions::default(),
            )
            .unwrap();
        let failure = ready(operation.execute()).unwrap_err();
        assert_eq!(state, failure.state());
        assert_eq!(5, failure.written_bytes());
        let cleanup_error = ready(operation.recovery_writer().unwrap().abort_async()).unwrap_err();
        assert_eq!(FsErrorKind::Io, cleanup_error.kind());
        assert!(operation.has_recovery_writer());
        assert_eq!(state, failure.state());
        assert_eq!(5, operation.written_bytes());
    }
    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        failing_stage: Some(AsyncCopyStage::WriterFlush),
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/flush").unwrap(),
            b"bytes".to_vec(),
            WriteOptions::default(),
        )
        .unwrap();
    let failure = ready(operation.execute()).unwrap_err();
    assert_eq!(5, failure.written_bytes());
    assert!(operation.has_recovery_writer());
}

#[test]
fn test_debug_omits_payload_and_consumed_failure_does_not_release_recovery() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::RetryableNotPublished),
        ..Default::default()
    });
    let mut operation = filesystem
        .begin_write_all(
            Path::parse("/safe-diagnostic").unwrap(),
            b"private-report-payload".to_vec(),
            WriteOptions::default(),
        )
        .unwrap();
    assert!(!format!("{operation:?}").contains("private-report-payload"));
    let error = ready(operation.execute()).unwrap_err().into_error();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert!(operation.has_recovery_writer());
    assert_eq!(
        AsyncWriteAllOperationState::Failed(WriteFailureState::RetryableNotPublished),
        operation.state()
    );
    assert_eq!(0, probe.writer_cancellations());
    assert!(!format!("{operation:?}").contains("private-report-payload"));
    let _outcome = ready(operation.recovery_writer().unwrap().abort_async()).unwrap();
}
