// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::time::SystemTime;

use qubit_fs::metadata::Checksum;
use qubit_fs::metadata::ChecksumAlgorithm;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::metadata::UserMetadata;

#[test]
fn test_is_directory_like_matches_directory_and_prefix() {
    assert!(FileMetadata::new(FileKind::Directory).is_directory_like());
    assert!(FileMetadata::new(FileKind::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileKind::File).is_directory_like());
}

#[test]
fn test_is_file_like_matches_file_and_object() {
    assert!(FileMetadata::new(FileKind::File).is_file_like());
    assert!(FileMetadata::new(FileKind::Object).is_file_like());
    assert!(!FileMetadata::new(FileKind::Directory).is_file_like());
}

#[test]
fn file_metadata_preserves_validated_provider_and_user_metadata() {
    let metadata = FileMetadata::new(FileKind::File).with_user_metadata(
        UserMetadata::new()
            .with("category", "private-category")
            .expect("ordinary user metadata key must be accepted"),
    );
    assert!(metadata.user_metadata().contains_key("category"));
    assert!(!format!("{metadata:?}").contains("private-category"));

    let provider = FileMetadata::new(FileKind::File).with_provider_metadata(
        UserMetadata::new()
            .with("storage_class", "private-tier")
            .expect("ordinary provider metadata key must be accepted"),
    );
    assert!(provider.provider_metadata().contains_key("storage_class"));
    assert!(!format!("{provider:?}").contains("private-tier"));

    assert!(UserMetadata::new().with("access_token", "plaintext").is_err());
}

/// Verifies optional metadata builders and accessors preserve every value.
#[test]
fn test_file_metadata_preserves_optional_fields() {
    let timestamp = SystemTime::UNIX_EPOCH;
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    let etag = ResourceVersion::from("etag-42");
    let metadata = FileMetadata::new(FileKind::File)
        .with_len(Some(0))
        .with_modified_at(Some(timestamp))
        .with_created_at(Some(timestamp))
        .with_accessed_at(Some(timestamp))
        .with_etag(Some(etag.clone()))
        .with_content_type(Some("text/plain".to_owned()))
        .with_checksum(Some(checksum.clone()));

    assert_eq!(Some(0), metadata.len());
    assert!(metadata.is_empty());
    assert_eq!(Some(timestamp), metadata.modified_at());
    assert_eq!(Some(timestamp), metadata.created_at());
    assert_eq!(Some(timestamp), metadata.accessed_at());
    assert_eq!(Some(&etag), metadata.etag());
    assert_eq!(Some("text/plain"), metadata.content_type());
    assert_eq!(Some(&checksum), metadata.checksum());
    assert_eq!(&FileKind::File, metadata.kind());
}
