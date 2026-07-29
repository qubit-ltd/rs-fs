// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for deletion outcomes.

use qubit_fs::DeleteOutcome;

/// Verifies outcomes preserve whether an accepted target was already missing.
#[test]
fn test_delete_outcome_reports_missing_state() {
    assert!(!DeleteOutcome::new(false).already_missing());
    assert!(DeleteOutcome::new(true).already_missing());
}
