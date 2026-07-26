// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error as _;

use qubit_fs::{FsError, FsErrorKind, FsOperation, PersistFailure, PersistFailureState};

const SECRET_SOURCE_TEXT: &str = "authorization=secret-token";

struct SecretSourceError;

impl std::fmt::Debug for SecretSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(SECRET_SOURCE_TEXT)
    }
}

impl std::fmt::Display for SecretSourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(SECRET_SOURCE_TEXT)
    }
}

impl std::error::Error for SecretSourceError {}

#[test]
fn persist_failure_exposes_progress_cause_display_and_owned_error() {
    let failure = PersistFailure::new(
        FsError::with_source(
            FsErrorKind::Io,
            FsOperation::PersistTemp,
            "persist failed",
            std::io::Error::other("storage error"),
        ),
        PersistFailureState::PublishedSourceRetained,
    );

    assert_eq!(
        PersistFailureState::PublishedSourceRetained,
        failure.state(),
    );
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.to_string().contains("PublishedSourceRetained"));
    assert!(failure.source().is_some());

    let error = failure.into_error();
    assert_eq!(FsOperation::PersistTemp, error.operation());
    assert!(error.source().is_some());
}

#[test]
fn persist_failure_debug_does_not_expand_secret_fs_error_source() {
    let failure = PersistFailure::new(
        FsError::with_source(
            FsErrorKind::Io,
            FsOperation::PersistTemp,
            "provider persist failed",
            SecretSourceError,
        ),
        PersistFailureState::Indeterminate,
    );

    let debug = format!("{failure:?}");
    assert!(!debug.contains(SECRET_SOURCE_TEXT));
    assert!(debug.contains("source_present"));
}
