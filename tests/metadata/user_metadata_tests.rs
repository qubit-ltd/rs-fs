// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::UserMetadata;

/// Verifies metadata formatting exposes keys without values.
#[test]
fn test_user_metadata_debug_hides_values() {
    let metadata = UserMetadata::new()
        .with("language", "private-value")
        .expect("the key should be safe");
    let debug = format!("{metadata:?}");
    assert!(debug.contains("language"));
    assert!(!debug.contains("private-value"));
    assert!(!metadata.is_empty());
    assert!(metadata.contains_key("language"));
    assert_eq!(
        vec![("language", "private-value")],
        metadata.iter().collect::<Vec<_>>()
    );
}

/// Verifies metadata keys are classified directly instead of through a URI.
#[test]
fn test_user_metadata_accepts_keys_with_uri_delimiters() {
    let metadata = UserMetadata::new()
        .with("password=like", "ordinary-value")
        .expect("a key containing URI delimiters is not itself sensitive");
    assert!(metadata.contains_key("password=like"));
    assert!(UserMetadata::new().with("password", "secret").is_err());
}
