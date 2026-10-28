// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::write::AsyncWriteAllOperationState;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteFailureState;
use qubit_fs_documentation_examples::async_recovery::WriteReportError;
use qubit_fs_documentation_examples::async_recovery::write_report;
use qubit_fs_documentation_examples::provider_minimal::read_health;
use qubit_fs_documentation_examples::provider_minimal_async::stat_health;

#[path = "../../../common/async_recording_spi.rs"]
mod async_recording_spi;
#[path = "../../../common/poll_support.rs"]
mod poll_support;

use async_recording_spi::AsyncCopyStage;
use async_recording_spi::AsyncRecordingConfig;
use async_recording_spi::async_recording_file_system;
use poll_support::ready;

#[test]
fn test_provider_guide_example_reads_health_resource() {
    assert_eq!(b"ok", read_health().unwrap().as_slice());
}

/// The async provider example resolves through the facade without a runtime.
#[test]
fn test_provider_guide_async_example_reads_health_metadata() {
    let metadata = ready(stat_health()).expect("health metadata must resolve");
    assert_eq!(Some(2), metadata.len());
}

#[test]
fn test_async_example_retains_failure_and_failed_cleanup() {
    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::Published),
        writer_abort_failure: Some(FsErrorKind::Io),
        ..Default::default()
    });
    let result = ready(write_report(
        &filesystem,
        Path::parse("/report").unwrap(),
        b"bytes".to_vec(),
        std::future::pending(),
    ));
    let Err(WriteReportError::Recovery(recovery)) = result else {
        panic!("expected recovery");
    };
    assert_eq!(WriteFailureState::Published, recovery.primary.as_ref().unwrap().state());
    assert!(recovery.cleanup_error.is_some());
    assert!(recovery.cleanup_outcome.is_none());
    assert!(recovery.operation.has_recovery());
}

#[test]
fn test_async_example_preserves_each_successful_cleanup_outcome() {
    for outcome in [
        WriteAbortOutcome::NotPublished,
        WriteAbortOutcome::Published,
        WriteAbortOutcome::Indeterminate,
    ] {
        let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
            writer_commit_failure: Some(WriteFailureState::Published),
            writer_abort_outcome: Some(outcome),
            ..Default::default()
        });
        let result = ready(write_report(
            &filesystem,
            Path::parse("/report").unwrap(),
            b"bytes".to_vec(),
            std::future::pending(),
        ));
        let Err(WriteReportError::Recovery(recovery)) = result else {
            panic!("expected recovery");
        };
        assert_eq!(Some(outcome), recovery.cleanup_outcome);
        assert!(recovery.cleanup_error.is_none());
        assert!(recovery.operation.has_recovery());
    }
}

#[test]
fn test_async_example_cancellation_keeps_operation_without_writer() {
    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        pending_stage: Some(AsyncCopyStage::OpenWriter),
        ..Default::default()
    });
    let result = ready(write_report(
        &filesystem,
        Path::parse("/report").unwrap(),
        b"bytes".to_vec(),
        std::future::ready(()),
    ));
    let Err(WriteReportError::Recovery(recovery)) = result else {
        panic!("expected recovery");
    };
    assert!(recovery.primary.is_none());
    assert!(recovery.cleanup_outcome.is_none());
    assert!(recovery.cleanup_error.is_none());
    assert!(!recovery.operation.has_recovery());
    assert_eq!(
        AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate),
        recovery.operation.state()
    );
    assert_eq!("/report", recovery.operation.path().to_string());
}

#[test]
fn test_async_example_success_publishes() {
    let (filesystem, _) = async_recording_file_system(Default::default());
    ready(write_report(
        &filesystem,
        Path::parse("/report").unwrap(),
        b"bytes".to_vec(),
        std::future::pending(),
    ))
    .unwrap();
}
