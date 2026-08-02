// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_io::Output;

/// Keeps abort publication certainty as a first-class provider result.
#[test]
fn test_write_abort_outcome_exposes_all_publication_states() {
    assert_ne!(
        qubit_fs::WriteAbortOutcome::NotPublished,
        qubit_fs::WriteAbortOutcome::Published,
    );
    assert_ne!(
        qubit_fs::WriteAbortOutcome::Published,
        qubit_fs::WriteAbortOutcome::Indeterminate,
    );
}

#[test]
fn test_write_all_commit_failure_retains_open_writer_for_recovery() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(true, Vec::new());
    let failure = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
        .expect_err("injected commit failure should propagate");
    assert_eq!(qubit_fs::FsErrorKind::Io, failure.error().kind());
    assert_eq!(
        qubit_fs::WriterState::Open,
        failure.writer().expect("writer should be retained").state(),
    );
}

/// Exercises the concrete synchronous writer's normal byte-transfer and
/// terminal-state behavior through the public filesystem facade.
#[test]
fn test_open_writer_transfers_bytes_and_rejects_closed_operations() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let target = qubit_fs::Path::parse("/target").expect("path should parse");
    let mut writer = filesystem
        .open_writer(&target, qubit_fs::WriteOptions::default())
        .expect("writer should open");
    assert_eq!(&target, writer.info().path());
    assert_eq!(qubit_fs::WriterState::Open, writer.state());
    assert!(!writer.is_buffered());
    assert!(format!("{writer:?}").contains("FileWriter"));
    Output::write_fully(&mut writer, b"bytes")
        .expect("writer should accept bytes");
    writer.flush().expect("writer should flush");
    writer.commit().expect("writer should commit");
    assert_eq!(qubit_fs::WriterState::Committed, writer.state());
    let commit = writer
        .commit()
        .expect_err("committed writer must reject a second commit");
    assert_eq!(qubit_fs::FsErrorKind::InvalidState, commit.error().kind());
    let write = Output::write_fully(&mut writer, b"bytes")
        .expect_err("committed writer must reject byte transfer");
    assert!(write.to_string().contains("writer no longer accepts bytes"));
    let flush = writer
        .flush()
        .expect_err("committed writer must reject flush");
    assert!(flush.to_string().contains("writer no longer accepts bytes"));
    let abort = writer
        .abort()
        .expect_err("committed writer must reject abort");
    assert_eq!(qubit_fs::FsErrorKind::InvalidState, abort.kind());
}

/// Preserves typed write-failure ownership, display, and error-source access.
#[test]
fn test_write_failure_exposes_error_state_and_parts() {
    let failure = qubit_fs::WriteFailure::new(
        qubit_fs::FsError::new(
            qubit_fs::FsErrorKind::Io,
            qubit_fs::FsOperation::CommitWriter,
            "commit failed",
        ),
        qubit_fs::WriteFailureState::RetryableNotPublished,
    );
    assert_eq!(
        qubit_fs::WriteFailureState::RetryableNotPublished,
        failure.state()
    );
    assert_eq!(qubit_fs::FsErrorKind::Io, failure.error().kind());
    assert!(failure.to_string().contains("commit failed"));
    assert!(std::error::Error::source(&failure).is_some());
    let (error, state) = failure.into_parts();
    assert_eq!(qubit_fs::FsErrorKind::Io, error.kind());
    assert_eq!(qubit_fs::WriteFailureState::RetryableNotPublished, state);

    let error = qubit_fs::WriteFailure::new(
        qubit_fs::FsError::new(
            qubit_fs::FsErrorKind::Io,
            qubit_fs::FsOperation::CommitWriter,
            "commit failed",
        ),
        qubit_fs::WriteFailureState::NotPublished,
    )
    .into_error();
    assert_eq!(qubit_fs::FsErrorKind::Io, error.kind());
}

/// Maps each provider commit certainty into the writer lifecycle and allows
/// cleanup from every failed publication state.
#[test]
fn test_open_writer_preserves_provider_commit_and_abort_states() {
    for (failure_state, expected_state) in [
        (
            qubit_fs::WriteFailureState::NotPublished,
            qubit_fs::WriterState::NotPublished,
        ),
        (
            qubit_fs::WriteFailureState::Published,
            qubit_fs::WriterState::Published,
        ),
        (
            qubit_fs::WriteFailureState::Indeterminate,
            qubit_fs::WriterState::Indeterminate,
        ),
    ] {
        let filesystem = crate::handle_support::writer_lifecycle_filesystem(
            Some(failure_state),
            None,
        );
        let mut writer = filesystem
            .open_writer(
                &qubit_fs::Path::parse("/target").expect("path should parse"),
                qubit_fs::WriteOptions::default(),
            )
            .expect("writer should open");
        let error = writer
            .commit()
            .expect_err("configured commit failure should propagate");
        assert_eq!(failure_state, error.state());
        assert_eq!(expected_state, writer.state());
        let outcome = writer
            .abort()
            .expect("failed writer should allow abort");
        let expected_after_abort = match outcome {
            qubit_fs::WriteAbortOutcome::NotPublished => {
                qubit_fs::WriterState::Aborted
            }
            qubit_fs::WriteAbortOutcome::Published => {
                qubit_fs::WriterState::Published
            }
            qubit_fs::WriteAbortOutcome::Indeterminate => {
                qubit_fs::WriterState::Indeterminate
            }
        };
        assert_eq!(expected_after_abort, writer.state());
        let repeated = writer
            .abort()
            .expect_err("completed abort must reject a second abort");
        assert_eq!(qubit_fs::FsErrorKind::InvalidState, repeated.kind());
    }
}

/// Preserves an indeterminate abort result and leaves a definite abort failure
/// in its previous state for explicit recovery.
#[test]
fn test_open_writer_abort_failure_tracks_certainty() {
    for (kind, expected_state) in [
        (qubit_fs::FsErrorKind::Io, qubit_fs::WriterState::Open),
        (
            qubit_fs::FsErrorKind::Indeterminate,
            qubit_fs::WriterState::Indeterminate,
        ),
    ] {
        let filesystem = crate::handle_support::writer_lifecycle_filesystem(
            None,
            Some(kind),
        );
        let mut writer = filesystem
            .open_writer(
                &qubit_fs::Path::parse("/target").expect("path should parse"),
                qubit_fs::WriteOptions::default(),
            )
            .expect("writer should open");
        let error = writer
            .abort()
            .expect_err("configured abort failure should propagate");
        assert_eq!(kind, error.kind());
        assert_eq!(expected_state, writer.state());
    }
}

/// Rejects a provider write outcome that downgrades an atomic-required
/// publication after the writer has reached the published state.
#[test]
fn test_open_writer_rejects_non_atomic_required_commit_outcome() {
    let filesystem =
        crate::handle_support::non_atomic_temp_directory_filesystem();
    let mut writer = filesystem
        .open_writer(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            qubit_fs::WriteOptions::default()
                .with_atomicity(qubit_fs::AtomicityRequirement::Required),
        )
        .expect("writer should open with advertised atomic capability");
    let error = writer
        .commit()
        .expect_err("non-atomic provider outcome must violate the contract");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.error().kind()
    );
    assert_eq!(qubit_fs::WriterState::Published, writer.state());
}
