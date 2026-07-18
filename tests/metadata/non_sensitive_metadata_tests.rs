// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::HashMap;

use qubit_fs::{
    FsErrorKind,
    NonSensitiveMetadata,
};
use qubit_metadata::Metadata;

#[test]
fn non_sensitive_metadata_rejects_sensitive_keys_at_every_supported_depth() {
    let cases = [
        Metadata::new().with("database_password", "plaintext".to_owned()),
        Metadata::new().with(
            "connection",
            HashMap::from([("api_token".to_owned(), "plaintext".to_owned())]),
        ),
        Metadata::new().with(
            "provider",
            serde_json::json!({
                "diagnostics": [
                    {"status": "failed"},
                    {"x-amz-signature": "plaintext"}
                ]
            }),
        ),
    ];

    for metadata in cases {
        let error = NonSensitiveMetadata::try_from(metadata)
            .expect_err("credential-like keys must be rejected");
        assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    }
}

#[test]
fn non_sensitive_metadata_accepts_safe_keys_and_debug_omits_values() {
    const VALUE_NOT_FOR_AUTOMATIC_LOGGING: &str =
        "https://storage.example/object?opaque=private-value";
    let metadata = Metadata::new()
        .with("endpoint", VALUE_NOT_FOR_AUTOMATIC_LOGGING.to_owned())
        .with(
            "labels",
            HashMap::from([("region".to_owned(), "private-region".to_owned())]),
        )
        .with(
            "provider",
            serde_json::json!({"replicas": [{"region": "primary"}]}),
        );
    let safe = NonSensitiveMetadata::try_from(metadata.clone())
        .expect("safe structural keys should be accepted");

    assert_eq!(
        Some(VALUE_NOT_FOR_AUTOMATIC_LOGGING.to_owned()),
        safe.get("endpoint")
    );
    assert_eq!(&metadata, safe.as_metadata());
    let as_ref: &Metadata = safe.as_ref();
    assert_eq!(&metadata, as_ref);
    assert!(!format!("{safe:?}").contains(VALUE_NOT_FOR_AUTOMATIC_LOGGING));
    assert_eq!(metadata, safe.clone().into_metadata());
    assert_eq!(metadata, Metadata::from(safe));
    assert!(NonSensitiveMetadata::new().is_empty());
}
