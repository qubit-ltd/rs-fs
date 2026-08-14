// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External fallback failures and cancellation behavior for asynchronous copy.

use qubit_fs::AchievedAtomicity;
use qubit_fs::AsyncCopyOperationState;
use qubit_fs::AtomicityRequirement;
use qubit_fs::CopyConflictPolicy;
use qubit_fs::CopyFailureState;
use qubit_fs::CopyOptions;
use qubit_fs::DurabilityRequirement;
use qubit_fs::FileSystemCapability;
use qubit_fs::FsErrorKind;
use qubit_fs::MetadataPreservePolicy;
use qubit_fs::Path;
use qubit_fs::ServerSidePreference;
use qubit_fs::WriteFailureState;
use qubit_fs::WriterState;

use crate::async_recording_spi::AsyncCopyStage;
use crate::async_recording_spi::AsyncRecordingConfig;
use crate::async_recording_spi::async_recording_file_system;
use crate::poll_support::assert_pending;
use crate::poll_support::ready;

/// Returns a stable absolute path for copy scenarios.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Covers every failed streamed I/O stage while retaining the recovery writer.
#[test]
fn test_async_stream_fallback_failures_retain_recovery_writer() {
    for (stage, expected, writer_state) in [
        (
            AsyncCopyStage::ReaderRead,
            CopyFailureState::Unchanged,
            WriterState::Open,
        ),
        (
            AsyncCopyStage::WriterWrite,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
        (
            AsyncCopyStage::WriterFlush,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
        (
            AsyncCopyStage::WriterCommit,
            CopyFailureState::Unchanged,
            WriterState::NotPublished,
        ),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                failing_stage: Some(stage),
                ..AsyncRecordingConfig::default()
            });
        let mut operation = file_system
            .begin_copy(
                path("/source"),
                path("/target"),
                CopyOptions::default(),
            )
            .expect("preflight should succeed");
        let failure =
            ready(operation.execute()).expect_err("injected stage should fail");
        assert_eq!(expected, failure.state());
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert!(
            operation.has_recovery_writer(),
            "{stage:?} should retain writer"
        );
        assert_eq!(
            AsyncCopyOperationState::Failed(expected),
            operation.state()
        );
        assert_eq!(
            writer_state,
            operation
                .recovery_writer()
                .expect("failed fallback retains its writer")
                .state()
        );
    }
}

/// Maps provider-confirmed writer publication certainty into the copy recovery
/// state when the declined native copy reaches commit.
#[test]
fn test_async_stream_fallback_commit_failure_preserves_certainty() {
    for (writer_failure, expected, writer_state) in [
        (
            WriteFailureState::RetryableNotPublished,
            CopyFailureState::Unchanged,
            WriterState::Open,
        ),
        (
            WriteFailureState::NotPublished,
            CopyFailureState::Unchanged,
            WriterState::NotPublished,
        ),
        (
            WriteFailureState::Published,
            CopyFailureState::Published,
            WriterState::Published,
        ),
        (
            WriteFailureState::Indeterminate,
            CopyFailureState::Indeterminate,
            WriterState::Indeterminate,
        ),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                writer_commit_failure: Some(writer_failure),
                ..AsyncRecordingConfig::default()
            });
        let mut operation = file_system
            .begin_copy(
                path("/source"),
                path("/target"),
                CopyOptions::default(),
            )
            .expect("copy preflight should succeed");
        let failure = ready(operation.execute())
            .expect_err("writer commit failure should propagate");
        assert_eq!(expected, failure.state());
        assert_eq!(
            writer_state,
            operation
                .recovery_writer()
                .expect("failed commit retains its writer")
                .state()
        );
    }
}

/// Rejects every fallback-incompatible option after the provider explicitly
/// declines native copy, before opening either stream handle.
#[test]
fn test_async_declined_copy_rejects_incompatible_fallback_options() {
    let options = [
        CopyOptions::default().with_continue_on_error(true),
        CopyOptions::default()
            .with_preserve_metadata(MetadataPreservePolicy::Portable),
        CopyOptions::default().with_create_parent(true),
        CopyOptions::default().with_conflict(CopyConflictPolicy::Overwrite),
    ];
    for options in options {
        let (file_system, probe) =
            async_recording_file_system(AsyncRecordingConfig::default());
        let mut operation = file_system
            .begin_copy(path("/source"), path("/target"), options)
            .expect("these options pass copy preflight");
        let failure = ready(operation.execute()).expect_err(
            "declined native copy must reject this fallback option",
        );
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
        assert_eq!(vec!["try_copy"], probe.calls());
    }
}

/// Rejects a known source length over the write-session limit before opening
/// fallback handles or exposing a recovery writer.
#[test]
fn test_async_stream_fallback_rejects_known_length_over_limits_before_opening_handles()
 {
    let configs = [AsyncRecordingConfig {
        maximum_write_bytes: Some(4),
        ..AsyncRecordingConfig::default()
    }];
    for config in configs {
        let (file_system, probe) = async_recording_file_system(config);
        let mut operation = file_system
            .begin_copy(
                path("/source"),
                path("/target"),
                CopyOptions::default(),
            )
            .expect("copy preflight should succeed before source stat");
        let failure = ready(operation.execute())
            .expect_err("known source length over a stream limit must fail");
        assert_eq!(FsErrorKind::ResourceLimitExceeded, failure.error().kind());
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert!(!operation.has_recovery_writer());
        assert_eq!(vec!["try_copy", "stat"], probe.calls());
    }
}
#[test]
fn test_async_stream_fallback_ignores_range_read_limit() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            maximum_read_range_bytes: Some(4),
            ..AsyncRecordingConfig::default()
        });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("copy preflight should succeed before source stat");
    let outcome = ready(operation.execute())
        .expect("sequential fallback should not use the range-read limit");
    assert_eq!(5, outcome.stats().bytes);
    assert_eq!(
        vec!["try_copy", "stat", "open_reader", "open_writer"],
        probe.calls()
    );
}

/// Uses the asynchronous stream fallback when copy is not advertised.
#[test]
fn test_async_copy_fallback_does_not_require_copy_capability() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            omitted_capability: Some(FileSystemCapability::Copy),
            decline_copy: true,
            ..AsyncRecordingConfig::default()
        });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("read and write capabilities should enable fallback");
    let outcome = ready(operation.execute())
        .expect("asynchronous stream fallback should complete");

    assert!(outcome.used_fallback());
    assert_eq!(vec!["stat", "open_reader", "open_writer"], probe.calls());
}

/// Applies the same no-fallback rule to requirements that pass normal copy
/// preflight because the provider advertises the corresponding capability.
#[test]
fn test_async_declined_copy_rejects_required_fallback_guarantees() {
    let cases = [
        (
            AsyncRecordingConfig {
                completed_copy: Some(AchievedAtomicity::Atomic),
                decline_copy: true,
                ..AsyncRecordingConfig::default()
            },
            CopyOptions::default()
                .with_atomicity(AtomicityRequirement::Required)
                .with_conflict(CopyConflictPolicy::Skip),
        ),
        (
            AsyncRecordingConfig {
                completed_copy: Some(AchievedAtomicity::Atomic),
                decline_copy: true,
                ..AsyncRecordingConfig::default()
            },
            CopyOptions::default()
                .with_durability(DurabilityRequirement::Required),
        ),
        (
            AsyncRecordingConfig {
                server_side_copy: true,
                decline_copy: true,
                ..AsyncRecordingConfig::default()
            },
            CopyOptions::default()
                .with_server_side(ServerSidePreference::Require),
        ),
    ];
    for (config, options) in cases {
        let (file_system, probe) = async_recording_file_system(config);
        let mut operation = file_system
            .begin_copy(path("/source"), path("/target"), options)
            .expect("capability should make preflight succeed");
        let failure = ready(operation.execute()).expect_err(
            "declined native copy must reject the required guarantee",
        );
        assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
        assert_eq!(CopyFailureState::Unchanged, failure.state());
        assert_eq!(vec!["try_copy"], probe.calls());
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
        let (file_system, probe) =
            async_recording_file_system(AsyncRecordingConfig {
                pending_stage: Some(stage),
                ..AsyncRecordingConfig::default()
            });
        let mut operation = file_system
            .begin_copy(
                path("/source"),
                path("/target"),
                CopyOptions::default(),
            )
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
        assert_eq!(
            WriterState::Indeterminate,
            operation
                .recovery_writer()
                .expect("cancelled fallback should retain its writer")
                .state(),
            "{stage:?} may have started writer I/O"
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
fn test_async_native_copy_cancellation_is_indeterminate_without_recovery_writer()
 {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
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
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
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
            CopyOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect("preflight should succeed");
    let failure = ready(operation.execute())
        .expect_err("downgraded completed copy must fail");
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
            CopyOptions::default()
                .with_server_side(ServerSidePreference::Require),
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
            CopyOptions::default()
                .with_preserve_metadata(MetadataPreservePolicy::Portable),
        )
        .expect("metadata policy needs no capability preflight");
    let failure = ready(operation.execute())
        .expect_err("missing metadata preservation must be rejected");
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
    let failure = ready(operation.execute())
        .expect_err("provider failure should propagate");
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
    assert_eq!(Some("async-recording"), failure.error().provider());
}

/// Verifies asynchronous copy failures are safely formattable and can be
/// consumed into the owned recovery facts required by a caller.
#[test]
fn test_async_copy_failure_exposes_owned_error_state_and_stats() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        copy_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let mut operation = file_system
        .begin_copy(path("/source"), path("/target"), CopyOptions::default())
        .expect("preflight should succeed");
    let failure = ready(operation.execute())
        .expect_err("provider failure should propagate");
    assert!(format!("{failure}").contains("injected"));
    assert!(format!("{failure:?}").contains("AsyncCopyFailure"));
    assert_eq!(0, failure.partial_stats().bytes);
    let as_error: &dyn std::error::Error = &failure;
    let source = as_error
        .source()
        .expect("Display/Error source should be available");
    assert!(
        source.to_string().contains("injected"),
        "source error should be preserved"
    );

    let (error, state, stats) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(0, stats.bytes);
}

/// Taking and dropping an indeterminate recovery writer does not request
/// unconfirmed provider cancellation.
#[test]
fn test_async_indeterminate_recovery_writer_drop_skips_cancellation() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
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
