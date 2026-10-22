// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI write failure tests.

use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::write::WriteFailureState;

/// Verifies write failure facts remain available through the public API.
#[test]
fn test_spi_write_failure_preserves_typed_state() {
    let failure = SpiWriteFailure::new(
        FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "commit failed"),
        WriteFailureState::Published,
    );
    assert_eq!(FsErrorKind::Io, failure.error().kind());
    assert_eq!(WriteFailureState::Published, failure.state());

    let (error, state) = failure.into_parts();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(WriteFailureState::Published, state);
}
