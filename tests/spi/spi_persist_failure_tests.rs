// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! SPI persistence failure tests.

use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::temp::PersistFailureState;

/// Verifies persistence failure facts remain available through the public API.
#[test]
fn test_spi_persist_failure_preserves_typed_state() {
    let failure = SpiPersistFailure::new(
        FsError::new(FsErrorKind::AlreadyExists, FsOperation::PersistTemp, "target exists"),
        PersistFailureState::NotPublished,
    );
    assert_eq!(FsErrorKind::AlreadyExists, failure.error().kind());
    assert_eq!(PersistFailureState::NotPublished, failure.state());

    let (error, state) = failure.into_parts();
    assert_eq!(FsErrorKind::AlreadyExists, error.kind());
    assert_eq!(PersistFailureState::NotPublished, state);
}
