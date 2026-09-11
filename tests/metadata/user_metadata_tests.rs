// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::UserMetadata;

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
    assert_eq!(vec![("language", "private-value")], metadata.iter().collect::<Vec<_>>());
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

#[test]
fn test_user_metadata_accessors_are_callable_directly() {
    let metadata = UserMetadata::new()
        .with("region", "private-region")
        .expect("safe key should be accepted");
    let get: for<'a> fn(&'a UserMetadata, &str) -> Option<&'a str> = UserMetadata::get;
    let is_empty: fn(&UserMetadata) -> bool = UserMetadata::is_empty;
    let contains_key: fn(&UserMetadata, &str) -> bool = UserMetadata::contains_key;

    assert_eq!(Some("private-region"), get(&metadata, "region"));
    assert!(!is_empty(&metadata));
    assert!(contains_key(&metadata, "region"));
    assert_eq!(vec![("region", "private-region")], metadata.iter().collect::<Vec<_>>());
}
