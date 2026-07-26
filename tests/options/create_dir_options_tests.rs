// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{CreateDirOptions, NonSensitiveMetadata, UserMetadata};

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
fn create_dir_options_preserve_validated_user_metadata() {
    let options = CreateDirOptions::default().with_user_metadata(
        UserMetadata::new()
            .with("category", "private-category")
            .unwrap(),
    );
    assert!(options.user_metadata.contains_key("category"));
    assert!(!format!("{options:?}").contains("private-category"));
}
