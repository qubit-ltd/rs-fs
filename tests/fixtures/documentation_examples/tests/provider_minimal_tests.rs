// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs_documentation_examples::provider_minimal::read_health;

#[test]
fn test_provider_guide_example_reads_health_resource() {
    assert_eq!(
        b"ok",
        read_health()
            .expect("health resource should be readable")
            .as_slice()
    );
}
