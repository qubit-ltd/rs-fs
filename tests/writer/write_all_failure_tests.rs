// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_io::Output;

#[test]
fn test_write_all_success_publishes_writer() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let outcome = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
        .expect("write should commit");
    assert_eq!(qubit_fs::AchievedAtomicity::Atomic, outcome.atomicity);
}

/// Verifies byte-stream diagnostics are retained as a source rather than
/// copied into the public filesystem error message.
#[test]
fn test_write_all_does_not_format_stream_error_message() {
    let filesystem = crate::handle_support::stream_failure_filesystem();
    let failure = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
        .expect_err("the injected stream write should fail");
    let (error, _) = failure.into_parts();
    assert!(!error.to_string().contains("top-secret"));
    assert!(error.source().is_some());
}

/// Verifies `write_all` rejects a finite write limit before opening a session.
#[test]
fn test_write_all_rejects_bytes_over_finite_write_limit() {
    let filesystem = crate::handle_support::limited_write_filesystem(3);
    let failure = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"four",
            qubit_fs::WriteOptions::default(),
        )
        .expect_err("the finite write limit should reject all bytes");
    let (error, writer) = failure.into_parts();
    assert_eq!(qubit_fs::FsErrorKind::ResourceLimitExceeded, error.kind());
    assert!(writer.is_none());
}

/// Verifies an open writer cannot exceed its cumulative finite write limit.
#[test]
fn test_open_writer_rejects_cumulative_bytes_over_finite_write_limit() {
    let filesystem = crate::handle_support::limited_write_filesystem(3);
    let mut writer = filesystem
        .open_writer(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            qubit_fs::WriteOptions::default(),
        )
        .expect("writer should open");
    Output::write_fully(&mut writer, b"two")
        .expect("the first write should fit the limit");
    let error = Output::write_fully(&mut writer, b"xx")
        .expect_err("the cumulative write should exceed the limit");
    assert!(error.to_string().contains("provider byte limit"));
}
