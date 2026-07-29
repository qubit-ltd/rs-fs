// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{NonSensitiveMetadata, UserMetadata};

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
