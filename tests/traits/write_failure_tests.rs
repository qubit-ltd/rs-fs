// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;

use qubit_fs::{FsError, FsErrorKind, FsOperation, WriteFailure, WriteFailureState};

#[test]
fn write_failure_preserves_error_and_recovery_state() {
    let failure = WriteFailure::new(
        FsError::new(
            FsErrorKind::Io,
            FsOperation::CommitWriter,
            "directory synchronization failed",
        ),
        WriteFailureState::Published,
    );

    assert_eq!(WriteFailureState::Published, failure.state());
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert!(failure.source().is_some());
    assert!(failure.to_string().contains("Published"));
    assert_eq!(FsOperation::CommitWriter, failure.into_error().operation());
}
