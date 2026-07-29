// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{FsError, FsErrorKind, FsOperation, FsResult};

fn succeed() -> FsResult<()> {
    Ok(())
}

fn fail() -> FsResult<()> {
    Err(FsError::new(FsErrorKind::Io, FsOperation::Exists, "failed"))
}

#[test]
fn test_fs_result_alias_accepts_success_values() {
    assert!(succeed().is_ok());
}

#[test]
fn test_fs_result_alias_accepts_fs_error_values() {
    assert_eq!(
        FsErrorKind::Io,
        fail().expect_err("call should fail").kind()
    );
}
