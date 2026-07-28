// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! External SPI-bound temporary resource behavior tests.

use qubit_fs::{
    AtomicityRequirement,
    FsErrorKind,
    Path,
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
    assert_eq!(TempResourceState::Indeterminate, temp.state());
}
