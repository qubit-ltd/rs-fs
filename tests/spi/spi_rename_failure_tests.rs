// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI rename failure tests.

use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::rename::RenameFailureState;
use qubit_fs::spi::SpiRenameFailure;

#[test]
fn test_failure_facts_are_executed_at_runtime() {
    let failure = SpiRenameFailure::new(
        FsError::new(FsErrorKind::NotFound, FsOperation::Rename, "missing source"),
        RenameFailureState::Indeterminate,
    );
    assert_eq!(failure.error().kind(), FsErrorKind::NotFound);
    assert_eq!(failure.state(), RenameFailureState::Indeterminate);

    let (error, state) = failure.into_parts();
    assert_eq!(error.kind(), FsErrorKind::NotFound);
    assert_eq!(state, RenameFailureState::Indeterminate);
}
