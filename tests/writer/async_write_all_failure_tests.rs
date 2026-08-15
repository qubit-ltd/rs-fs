// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;

use crate::async_recording_spi::AsyncRecordingConfig;
use crate::async_recording_spi::async_recording_file_system;
use crate::poll_support::ready;

#[test]
fn test_async_write_all_failure_exposes_recovery_and_formatting() {
    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        writer_commit_failure: Some(WriteFailureState::NotPublished),
        ..AsyncRecordingConfig::default()
    });
    let mut failure = ready(filesystem.write_all(
        &Path::parse("/target").expect("path should parse"),
        b"bytes",
        WriteOptions::default(),
    ))
    .expect_err("commit failure should retain an async writer");

    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.writer().is_some());
    assert!(failure.writer_mut().is_some());
    assert!(failure.to_string().contains("commit failure"));
    assert!(format!("{failure:?}").contains("has_writer"));
    assert!(Error::source(&failure).is_some());

    let (error, writer) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert!(writer.is_some());
}
