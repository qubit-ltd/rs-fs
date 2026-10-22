// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::FileSystemId;

#[test]
fn test_file_system_id_preserves_validated_identity_text() {
    let id = FileSystemId::new("local").expect("identity should be valid");

    assert_eq!("local", id.as_str());
    assert_eq!("local", id.to_string());
}

#[test]
fn test_file_system_id_rejects_empty_identity_text() {
    assert!(FileSystemId::new("").is_err());
}

#[test]
fn test_file_system_id_rejects_line_breaks_in_identity_text() {
    assert!(FileSystemId::new("bad\nidentity").is_err());
}
