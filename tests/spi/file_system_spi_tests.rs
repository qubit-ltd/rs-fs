// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI failure state tests.

use qubit_fs::spi::{
    SpiCopyFailure,
    SpiRenameFailure,
};
use qubit_fs::{
    CopyFailureState,
    CopyStats,
    FsError,
    FsErrorKind,
    FsOperation,
    RenameFailureState,
};

/// Verifies provider copy failures retain typed recovery state and statistics.
#[test]
fn test_spi_copy_failure_preserves_typed_state() {
    let failure = SpiCopyFailure::new(
        FsError::new(
            FsErrorKind::Indeterminate,
            FsOperation::Copy,
            "test failure",
        ),
        CopyFailureState::Indeterminate,
        CopyStats::default(),
    );
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    let (_, state, stats) = failure.into_parts();
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(CopyStats::default(), stats);
}

/// Verifies provider rename failures retain typed recovery state.
#[test]
fn test_spi_rename_failure_preserves_typed_state() {
    let failure = SpiRenameFailure::new(
        FsError::new(
            FsErrorKind::Indeterminate,
            FsOperation::Rename,
            "test failure",
        ),
        RenameFailureState::Indeterminate,
    );
    assert_eq!(RenameFailureState::Indeterminate, failure.state());
    let (_, state) = failure.into_parts();
    assert_eq!(RenameFailureState::Indeterminate, state);
}
