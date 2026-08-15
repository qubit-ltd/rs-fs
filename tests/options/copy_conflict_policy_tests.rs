// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::copy::CopyConflictPolicy;

#[test]
fn test_copy_conflict_policy_default_is_fail() {
    assert_eq!(CopyConflictPolicy::Fail, CopyConflictPolicy::default());
}
