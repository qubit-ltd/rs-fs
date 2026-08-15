// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for directory-creation outcomes.

use qubit_fs::directory::CreateDirectoryOutcome;

/// Verifies outcomes preserve whether the target already existed.
#[test]
fn test_create_directory_outcome_reports_existing_state() {
    assert!(!CreateDirectoryOutcome::new(false).already_existed());
    assert!(CreateDirectoryOutcome::new(true).already_existed());
}

/// Verifies outcomes retain the provider-reported ancestor count.
#[test]
fn test_create_directory_outcome_reports_created_ancestors() {
    let outcome = CreateDirectoryOutcome::new(false).with_created_ancestors(3);

    assert_eq!(Some(3), outcome.created_ancestors());
}
