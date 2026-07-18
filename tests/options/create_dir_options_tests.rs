// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    CreateDirOptions,
    FsErrorKind,
    NonSensitiveMetadata,
};

#[test]
fn test_create_dir_options_full_configuration_is_usable() {
    let options = CreateDirOptions {
        recursive: true,
        exists_ok: true,
        user_metadata: NonSensitiveMetadata::new(),
    };

    assert!(options.recursive);
    assert!(options.exists_ok);
}

#[test]
fn create_dir_options_validate_user_metadata_keys() {
    let error = CreateDirOptions::default()
        .with_user_metadata(qubit_metadata::Metadata::new().with(
            "provider",
            serde_json::json!({"items": [{"private_key": "plaintext"}]}),
        ))
        .expect_err("nested credential metadata must be rejected");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());

    let options = CreateDirOptions::default()
        .with_user_metadata(
            qubit_metadata::Metadata::new()
                .with("category", "private-category".to_owned()),
        )
        .expect("safe metadata keys should be accepted");
    assert!(options.user_metadata.contains_key("category"));
    assert!(!format!("{options:?}").contains("private-category"));
}
