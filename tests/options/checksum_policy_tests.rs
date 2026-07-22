// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ChecksumPolicy;

#[test]
fn test_checksum_policy_default_is_none() {
    assert_eq!(ChecksumPolicy::None, ChecksumPolicy::default());
}
