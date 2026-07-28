// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regression coverage for asynchronous facade-handle contract boundaries.

#[allow(dead_code)]
#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[allow(dead_code)]
#[path = "common/poll_support.rs"]
mod poll_support;

use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    DirEntry,
    FileKind,
    FsErrorKind,
    ListOptions,
    Path,
    PersistOptions,
    TempFileOptions,
    WriteFailureState,
    WriteOptions,
    WriterState,
};

use crate::async_recording_spi::{
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::ready;

/// Parses an absolute path used by one regression scenario.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
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
        WriteOptions {
            atomicity: AtomicityRequirement::Required,
            ..WriteOptions::default()
        },
    ))
    .expect("provider advertises atomic write support");
    let error = ready(writer.commit_async()).expect_err(
        "a non-atomic success must not satisfy a required atomic write",
    );
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
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
    ready(writer.abort_async()).expect("cleanup should succeed");
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
    let failure = ready(temp.persist(
        &path("/final"),
        PersistOptions {
            atomicity: AtomicityRequirement::Required,
            ..PersistOptions::default()
        },
    ))
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
