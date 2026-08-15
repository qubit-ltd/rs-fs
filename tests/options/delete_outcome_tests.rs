// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for deletion outcomes.

use qubit_fs::directory::DeleteOutcome;

/// Verifies outcomes preserve whether an accepted target was already missing.
#[test]
fn test_delete_outcome_reports_missing_state() {
    assert!(!DeleteOutcome::new(false).already_missing());
    assert!(DeleteOutcome::new(true).already_missing());
}

/// Verifies outcomes retain the provider-reported deletion count.
#[test]
fn test_delete_outcome_reports_deleted_entries() {
    let outcome = DeleteOutcome::new(false).with_deleted_entries(4);

    assert_eq!(Some(4), outcome.deleted_entries());
}
