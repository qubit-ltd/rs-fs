use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::write::AsyncWriteAllOperationState;
use qubit_fs::write::WriteFailureState;

use super::async_recording_spi::AsyncCopyStage;
use super::async_recording_spi::AsyncRecordingConfig;
use super::async_recording_spi::async_recording_file_system;
use super::async_recovery::WriteReportError;
use super::async_recovery::write_report;
use super::poll_support::ready;

#[test]
fn async_example_retains_failure_and_failed_cleanup() {
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
    assert!(recovery.operation.has_recovery_writer());
}

#[test]
fn async_example_cancellation_keeps_operation_without_writer() {
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
    assert!(!recovery.operation.has_recovery_writer());
    assert_eq!(
        AsyncWriteAllOperationState::Failed(WriteFailureState::Indeterminate),
        recovery.operation.state()
    );
    assert_eq!("/report", recovery.operation.path().to_string());
}

#[test]
fn async_example_success_publishes() {
    let (filesystem, _) = async_recording_file_system(Default::default());
    ready(write_report(
        &filesystem,
        Path::parse("/report").unwrap(),
        b"bytes".to_vec(),
        std::future::pending(),
    ))
    .unwrap();
}
