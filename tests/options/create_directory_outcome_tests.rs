// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for directory-creation outcomes.

use qubit_fs::CreateDirectoryOutcome;

/// Verifies outcomes preserve whether the target already existed.
#[test]
fn test_create_directory_outcome_reports_existing_state() {
    assert!(!CreateDirectoryOutcome::new(false).already_existed());
    assert!(CreateDirectoryOutcome::new(true).already_existed());
}
