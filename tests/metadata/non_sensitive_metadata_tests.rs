// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::NonSensitiveMetadata;
use qubit_fs::metadata::UserMetadata;

#[test]
fn non_sensitive_metadata_preserves_safe_metadata_without_logging_values() {
    let metadata = UserMetadata::new()
        .with("endpoint", "private-value")
        .expect("safe keys should be accepted");
    let safe = NonSensitiveMetadata::from(metadata.clone());

    assert_eq!(Some("private-value"), safe.get("endpoint"));
    assert!(safe.contains_key("endpoint"));
    assert!(!safe.is_empty());
    assert_eq!(&metadata, safe.as_metadata());
    assert_eq!(&metadata, safe.as_ref());
    assert!(!format!("{safe:?}").contains("private-value"));
    assert_eq!(metadata, safe.clone().into_metadata());
    assert_eq!(metadata, UserMetadata::from(safe));
    assert!(NonSensitiveMetadata::new().is_empty());
}

#[test]
fn non_sensitive_metadata_accessors_are_callable_directly() {
    let metadata = NonSensitiveMetadata::from(
        UserMetadata::new()
            .with("endpoint", "private-value")
            .expect("safe keys should be accepted"),
    );
    let as_metadata: fn(&NonSensitiveMetadata) -> &UserMetadata = NonSensitiveMetadata::as_metadata;
    let is_empty: fn(&NonSensitiveMetadata) -> bool = NonSensitiveMetadata::is_empty;
    let contains_key: fn(&NonSensitiveMetadata, &str) -> bool = NonSensitiveMetadata::contains_key;
    let get: for<'a> fn(&'a NonSensitiveMetadata, &str) -> Option<&'a str> = NonSensitiveMetadata::get;

    assert_eq!(Some("private-value"), get(&metadata, "endpoint"));
    assert!(contains_key(&metadata, "endpoint"));
    assert!(!is_empty(&metadata));
    assert_eq!("private-value", as_metadata(&metadata).get("endpoint").unwrap());
}
