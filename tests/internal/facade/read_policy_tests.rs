// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::read::ReadOptions;

/// Verifies the public prefix-read facade stops at the requested bound.
#[test]
fn test_read_prefix_stops_at_requested_maximum() {
    let (file_system, _, _) = crate::handle_support::filesystem(false, Vec::new());
    let path = Path::parse("/source").expect("test path should parse");
    let bytes = file_system
        .read_prefix(&path, ReadOptions::default(), 3)
        .expect("bounded prefix read should succeed");
    assert_eq!(b"byt", bytes.as_slice());
}
