// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI copy failure tests.

use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyStats;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::spi::SpiCopyFailure;

/// Verifies copy failure facts remain available through the public API.
#[test]
fn test_spi_copy_failure_preserves_partial_progress() {
    let failure = SpiCopyFailure::new(
        FsError::new(FsErrorKind::NotFound, FsOperation::Copy, "missing source"),
        CopyFailureState::PartiallyPublished,
        CopyStats {
            bytes: 4,
            ..CopyStats::default()
        },
    );
    assert_eq!(FsErrorKind::NotFound, failure.error().kind());
    assert_eq!(CopyFailureState::PartiallyPublished, failure.state());

    let (error, state, stats) = failure.into_parts();
    assert_eq!(FsErrorKind::NotFound, error.kind());
    assert_eq!(CopyFailureState::PartiallyPublished, state);
    assert_eq!(4, stats.bytes);
}
