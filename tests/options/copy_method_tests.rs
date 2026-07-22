// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::CopyMethod;

#[test]
fn test_copy_method_variants_are_comparable() {
    let methods = [
        CopyMethod::Local,
        CopyMethod::ServerSide,
        CopyMethod::Stream,
        CopyMethod::Mixed,
    ];

    assert!(methods.contains(&CopyMethod::Local));
    assert!(methods.contains(&CopyMethod::ServerSide));
    assert!(methods.contains(&CopyMethod::Stream));
    assert!(methods.contains(&CopyMethod::Mixed));
}
