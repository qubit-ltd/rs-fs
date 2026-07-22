// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::MetadataPreservePolicy;

#[test]
fn test_metadata_preserve_policy_default_is_portable() {
    assert_eq!(
        MetadataPreservePolicy::Portable,
        MetadataPreservePolicy::default(),
    );
}
