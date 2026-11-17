// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::FsEffectCapacityError;
use qubit_fs::FsEffectObservationError;

#[test]
fn test_effect_errors_explain_the_failure() {
    assert_eq!(
        FsEffectCapacityError.to_string(),
        "filesystem effect fact capacity exhausted"
    );
    assert_eq!(
        FsEffectObservationError::Poisoned.to_string(),
        "filesystem effect fact lock is poisoned"
    );
}
