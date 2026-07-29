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
    UserMetadata,
};

#[test]
fn test_is_directory_like_matches_directory_and_prefix() {
    assert!(FileMetadata::new(FileKind::Directory).is_directory_like());
    assert!(FileMetadata::new(FileKind::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileKind::File).is_directory_like());
}

#[test]
fn file_metadata_preserves_validated_provider_and_user_metadata() {
    let metadata = FileMetadata::new(FileKind::File).with_user_metadata(
        UserMetadata::new()
            .with("category", "private-category")
            .unwrap(),
    );
    assert!(metadata.user_metadata.contains_key("category"));
    assert!(!format!("{metadata:?}").contains("private-category"));

    let provider = FileMetadata::new(FileKind::File).with_provider_metadata(
        UserMetadata::new()
            .with("storage_class", "private-tier")
            .unwrap(),
    );
    assert!(provider.provider_metadata.contains_key("storage_class"));
    assert!(!format!("{provider:?}").contains("private-tier"));

    assert!(
        UserMetadata::new()
            .with("access_token", "plaintext")
            .is_err()
    );
}
