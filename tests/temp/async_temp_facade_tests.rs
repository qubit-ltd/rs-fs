// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External SPI-bound temporary resource behavior tests.

use std::error::Error;

use qubit_fs::{
    AtomicityRequirement,
    FsErrorKind,
    Path,
    PersistFailureState,
    PersistOptions,
    TempDirectoryOptions,
    TempFileOptions,
    TempResourceState,
};

use crate::async_recording_spi::{
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::ready;

/// Returns a stable absolute destination used by persistence tests.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Rejects a provider-created temporary identity before a handle escapes the
/// facade.
#[test]
fn test_async_temp_creation_rejects_mismatched_provider_identity() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            invalid_temp_identity: true,
            ..AsyncRecordingConfig::default()
        });
    let Err(error) =
        ready(file_system.create_temp_file(TempFileOptions::default()))
    else {
        panic!("mismatched temporary identity must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(vec!["create_temp_file", "cleanup"], probe.calls());
}

/// Applies the same identity and compensating-cleanup contract to temporary
/// directories as it does to temporary files.
#[test]
fn test_async_temp_directory_rejects_mismatched_provider_identity() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            invalid_temp_identity: true,
            ..AsyncRecordingConfig::default()
        });
    let Err(error) = ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    ) else {
        panic!("mismatched temporary directory identity must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(vec!["create_temp_directory", "cleanup"], probe.calls());
}

/// Verifies failed invalid-session cleanup remains the inspectable source of
/// the facade contract failure.
#[test]
fn test_async_invalid_temp_identity_preserves_cleanup_error_source() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_identity: true,
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(error) =
        ready(file_system.create_temp_file(TempFileOptions::default()))
    else {
        panic!("invalid temporary identity must fail");
    };
    let cleanup = error.source().expect("cleanup error should be retained");
    assert!(cleanup.to_string().contains("injected cleanup failure"));
    assert!(cleanup.source().is_some());
}

/// Verifies persistence preflight fails before calling a temporary session.
#[test]
fn test_async_temp_persist_preflight_has_no_session_call() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut temp =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    let failure = ready(temp.persist(
        &path("/final"),
        PersistOptions {
            atomicity: AtomicityRequirement::Required,
            ..PersistOptions::default()
        },
    ))
    .expect_err("missing atomic persistence support must fail locally");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(TempResourceState::Owned, temp.state());
    assert_eq!(vec!["create_temp_file"], probe.calls());
}

/// Covers successful temporary file persistence and terminal state publication.
#[test]
fn test_async_temp_file_persist_delegates_after_preflight() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            atomic_temp_persist: true,
            ..AsyncRecordingConfig::default()
        });
    let mut temp =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    let outcome =
        ready(temp.persist(&path("/final"), PersistOptions::default()))
            .expect("persistence should succeed");
    assert_eq!(path("/final"), outcome.target);
    assert_eq!(TempResourceState::Persisted, temp.state());
    assert_eq!(vec!["create_temp_file", "persist"], probe.calls());
}

/// Rejects a provider outcome that reports a different persistence target.
#[test]
fn test_async_temp_file_rejects_mismatched_persist_target() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        ..AsyncRecordingConfig::default()
    });
    let mut temp =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");

    let failure = ready(
        temp.persist(&path("/wrong-persist-target"), PersistOptions::default()),
    )
    .expect_err("mismatched target should violate the provider contract");

    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(PersistFailureState::Indeterminate, failure.state());
    assert_eq!(TempResourceState::Indeterminate, temp.state());
}

/// Covers cleanup and keep delegation for both opaque temporary resource kinds.
#[test]
fn test_async_temp_cleanup_and_keep_delegate_to_spi_sessions() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut file =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    ready(file.cleanup()).expect("cleanup should succeed");
    assert_eq!(TempResourceState::Cleaned, file.state());
    let mut directory = ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    )
    .expect("temporary directory should open");
    ready(directory.keep()).expect("keep should succeed");
    assert_eq!(TempResourceState::Kept, directory.state());
    assert_eq!(
        vec![
            "create_temp_file",
            "cleanup",
            "create_temp_directory",
            "keep"
        ],
        probe.calls()
    );
}

/// Preserves indeterminate persistence instead of claiming cleanup is required.
#[test]
fn test_async_temp_persist_indeterminate_is_preserved() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        temp_persist_indeterminate: true,
        ..AsyncRecordingConfig::default()
    });
    let mut temp =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    let error = ready(temp.persist(&path("/final"), PersistOptions::default()))
        .expect_err("persistence should be indeterminate");
    assert_eq!(FsErrorKind::Indeterminate, error.error().kind());
    assert_eq!(Some(&path("/tmp/recording")), error.error().path(),);
    assert_eq!(Some(&path("/final")), error.error().target());
    assert_eq!(Some("async-recording"), error.error().provider());
    assert_eq!(TempResourceState::Indeterminate, temp.state());
}

/// Keeps definite lifecycle failures recoverable and makes cleanup failure
/// require a later explicit cleanup decision.
#[test]
fn test_async_temp_lifecycle_failures_preserve_expected_states() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        temp_keep_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let mut file =
        ready(file_system.create_temp_file(TempFileOptions::default()))
            .expect("temporary file should open");
    let keep = ready(file.keep())
        .expect_err("configured keep failure should propagate");
    assert_eq!(FsErrorKind::Io, keep.kind());
    assert_eq!(TempResourceState::Owned, file.state());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let mut directory = ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    )
    .expect("temporary directory should open");
    let cleanup = ready(directory.cleanup())
        .expect_err("configured cleanup failure should propagate");
    assert_eq!(FsErrorKind::Io, cleanup.kind());
    assert_eq!(TempResourceState::CleanupRequired, directory.state());
}

/// Exposes the temporary-directory wrapper's path, persistence, and terminal
/// lifecycle validation through the public asynchronous facade.
#[test]
fn test_async_temp_directory_persists_and_rejects_later_lifecycle_calls() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        ..AsyncRecordingConfig::default()
    });
    let mut directory = ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    )
    .expect("temporary directory should open");
    assert_eq!(&path("/tmp/recording"), directory.path());
    let outcome =
        ready(directory.persist(&path("/final"), PersistOptions::default()))
            .expect("temporary directory should persist");
    assert_eq!(path("/final"), outcome.target);
    assert_eq!(TempResourceState::Persisted, directory.state());
    let persist =
        ready(directory.persist(&path("/other"), PersistOptions::default()))
            .expect_err("persisted directory must reject a second persist");
    assert_eq!(FsErrorKind::InvalidState, persist.error().kind());
    let cleanup = ready(directory.cleanup())
        .expect_err("persisted directory must reject later cleanup");
    assert_eq!(FsErrorKind::InvalidState, cleanup.kind());
}

/// Retains the provider-confirmed state after both definite and partially
/// published temporary persistence failures.
#[test]
fn test_async_temp_persist_failure_states_drive_lifecycle() {
    for (failure_state, expected_state) in [
        (PersistFailureState::NotPublished, TempResourceState::Owned),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
        ),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                atomic_temp_persist: true,
                temp_persist_failure: Some(failure_state),
                ..AsyncRecordingConfig::default()
            });
        let mut file =
            ready(file_system.create_temp_file(TempFileOptions::default()))
                .expect("temporary file should open");
        let failure =
            ready(file.persist(&path("/final"), PersistOptions::default()))
                .expect_err("configured persist failure should propagate");
        assert_eq!(failure_state, failure.state());
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(expected_state, file.state());
    }
}
