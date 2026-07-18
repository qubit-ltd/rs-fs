// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    FileKind,
    FileMetadata,
    FsErrorKind,
};
use qubit_metadata::Metadata;

#[test]
fn test_is_directory_like_matches_directory_and_prefix() {
    assert!(FileMetadata::new(FileKind::Directory).is_directory_like());
    assert!(FileMetadata::new(FileKind::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileKind::File).is_directory_like());
}

#[test]
fn file_metadata_validates_provider_and_user_metadata() {
    let error = FileMetadata::new(FileKind::File)
        .with_provider_metadata(Metadata::new().with(
            "provider",
            serde_json::json!({"items": [{"credential": "plaintext"}]}),
        ))
        .expect_err("nested provider credentials must be rejected");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());

    let metadata = FileMetadata::new(FileKind::File)
        .with_user_metadata(
            Metadata::new().with("category", "private-category".to_owned()),
        )
        .expect("safe user metadata keys should be accepted");
    assert!(metadata.user_metadata.contains_key("category"));
    assert!(!format!("{metadata:?}").contains("private-category"));

    let provider = FileMetadata::new(FileKind::File)
        .with_provider_metadata(
            Metadata::new().with("storage_class", "private-tier".to_owned()),
        )
        .expect("safe provider metadata keys should be accepted");
    assert!(provider.provider_metadata.contains_key("storage_class"));
    assert!(!format!("{provider:?}").contains("private-tier"));

    let error = FileMetadata::new(FileKind::File)
        .with_user_metadata(
            Metadata::new().with("access_token", "plaintext".to_owned()),
        )
        .expect_err("credential-like user metadata keys must be rejected");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}
