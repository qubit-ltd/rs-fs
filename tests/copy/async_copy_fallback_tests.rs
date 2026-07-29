// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! External fallback failures and cancellation behavior for asynchronous copy.

use qubit_fs::{
    AchievedAtomicity, AsyncCopyOperationState, AtomicityRequirement, CopyFailureState,
    CopyOptions, FsErrorKind, MetadataPreservePolicy, Path, ServerSidePreference,
};

use crate::async_recording_spi::{
    AsyncCopyStage, AsyncRecordingConfig, async_recording_file_system,
};
use crate::poll_support::{assert_pending, ready};

/// Returns a stable absolute path for copy scenarios.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Covers every failed streamed I/O stage while retaining the recovery writer.
#[test]
fn test_async_stream_fallback_failures_retain_recovery_writer() {
    for stage in [
        AsyncCopyStage::ReaderRead,
        AsyncCopyStage::WriterWrite,
        AsyncCopyStage::WriterFlush,
        AsyncCopyStage::WriterCommit,
    ] {
        let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
            failing_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        let mut operation = file_system
            .begin_copy(path("/source"), path("/target"), CopyOptions::default())
            .expect("preflight should succeed");
        let failure = ready(operation.execute()).expect_err("injected stage should fail");
        assert_eq!(CopyFailureState::PartiallyPublished, failure.state());
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert!(
            operation.has_recovery_writer(),
            "{stage:?} should retain writer"
        );
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::PartiallyPublished),
            operation.state()
        );
    }
}

/// Covers cancellation after every fallback await point and preserves recovery.
#[test]
fn test_async_stream_fallback_cancellation_is_indeterminate_with_recovery() {
    for stage in [
        AsyncCopyStage::ReaderRead,
        AsyncCopyStage::WriterWrite,
        AsyncCopyStage::WriterFlush,
        AsyncCopyStage::WriterCommit,
    ] {
        let (file_system, probe) = async_recording_file_system(AsyncRecordingConfig {
            pending_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        let mut operation = file_system
            .begin_copy(path("/source"), path("/target"), CopyOptions::default())
            .expect("preflight should succeed");
        let mut future = Box::pin(operation.execute());
        assert_pending(future.as_mut());
        let calls_before_drop = probe.calls();
        drop(future);
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate),
            operation.state()
        );
        assert!(
            operation.has_recovery_writer(),
            "{stage:?} should retain writer"
        );
        drop(operation);
        assert_eq!(
            calls_before_drop,
            probe.calls(),
            "drop must not call the SPI"
        );
    }
}

/// Covers cancellation before fallback has allocated a writer.
#[test]
fn test_async_native_copy_cancellation_is_indeterminate_without_recovery_writer() {
    let (file_system, probe) = async_recording_file_system(AsyncRecordingConfig {
        pending_stage: Some(AsyncCopyStage::TryCopy),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("preflight should succeed");
    let mut future = Box::pin(operation.execute());
    assert_pending(future.as_mut());
    let calls_before_drop = probe.calls();
    drop(future);
    assert_eq!(
        AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate),
        operation.state()
    );
    assert!(!operation.has_recovery_writer());
    drop(operation);
    assert_eq!(calls_before_drop, probe.calls());
}

/// Verifies an unpolled execute future has no state or provider-I/O effect.
#[test]
fn test_dropping_unpolled_execute_future_keeps_operation_ready() {
    let (file_system, probe) = async_recording_file_system(AsyncRecordingConfig::default());
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("preflight should succeed");
    let future = operation.execute();
    drop(future);
    assert_eq!(AsyncCopyOperationState::Ready, operation.state());
    assert!(probe.calls().is_empty());
}

/// Rechecks a completed native outcome after provider I/O.
#[test]
fn test_async_completed_copy_rechecks_required_atomicity() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::NonAtomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions {
                atomicity: AtomicityRequirement::Required,
                ..CopyOptions::default()
            },
        )
        .expect("preflight should succeed");
    let failure = ready(operation.execute()).expect_err("downgraded completed copy must fail");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(CopyFailureState::Published, failure.state());
}

/// Verifies an asynchronous native completion cannot satisfy a required
/// server-side request merely because the provider advertises the capability.
#[test]
fn test_async_completed_native_copy_violates_required_server_side() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        server_side_copy: true,
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions {
                server_side: ServerSidePreference::Require,
                ..CopyOptions::default()
            },
        )
        .expect("server-side capability should pass preflight");
    let failure = ready(operation.execute())
        .expect_err("native completion cannot satisfy server-side requirement");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies asynchronous successful completion cannot omit a requested
/// metadata preservation fact.
#[test]
fn test_async_completed_copy_missing_metadata_is_contract_failure() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        completed_copy: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(
            path("/source"),
            path("/target"),
            CopyOptions {
                preserve_metadata: MetadataPreservePolicy::Portable,
                ..CopyOptions::default()
            },
        )
        .expect("metadata policy needs no capability preflight");
    let failure =
        ready(operation.execute()).expect_err("missing metadata preservation must be rejected");
    assert_eq!(CopyFailureState::Published, failure.state());
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
}

/// Verifies an asynchronous provider failure retains the requested source and
/// target instead of the facade root placeholder.
#[test]
fn test_async_native_copy_failure_has_source_and_target_context() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        copy_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/target");
    let mut operation = file_system
        .begin_copy(source.clone(), target.clone(), CopyOptions::default())
        .expect("preflight should succeed");
    let failure = ready(operation.execute()).expect_err("provider failure should propagate");
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
    assert_eq!(Some("async-recording"), failure.error().provider());
}

/// Taking and dropping an indeterminate recovery writer does not request
/// unconfirmed provider cancellation.
#[test]
fn test_async_indeterminate_recovery_writer_drop_skips_cancellation() {
    let (file_system, probe) = async_recording_file_system(AsyncRecordingConfig {
        failing_stage: Some(AsyncCopyStage::WriterFlush),
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("preflight should succeed");
    let _ = ready(operation.execute()).expect_err("flush should fail");
    let writer = operation
        .take_recovery_writer()
        .expect("writer should be retained");
    drop(writer);
    assert_eq!(0, probe.writer_cancellations());
}
