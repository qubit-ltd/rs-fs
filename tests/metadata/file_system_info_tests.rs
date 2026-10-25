// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::UserMetadata;
use qubit_fs::path::PathSemantics;

#[test]
fn test_file_system_info_is_a_validated_local_snapshot() {
    let id = FileSystemId::new("mock-instance").expect("valid filesystem identity must be accepted");
    let provider_id = "mock";
    let info = FileSystemInfo::new(id.clone(), provider_id, PathSemantics::ObjectKey)
        .with_scheme("mock")
        .expect("valid provider scheme must be accepted");

    assert_eq!(&id, info.id());
    assert_eq!(provider_id, info.provider_id());
    assert_eq!(&["mock"], info.schemes());
    assert_eq!(PathSemantics::ObjectKey, info.path_semantics());
    assert!(info.provider_metadata().is_empty());
    assert!(FileSystemId::new("").is_err());
}

#[test]
fn test_file_system_info_deduplicates_schemes_and_replaces_provider_metadata() {
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance").expect("valid filesystem identity must be accepted"),
        "mock",
        PathSemantics::ProviderSpecific,
    )
    .with_scheme("MOCK")
    .expect("valid uppercase scheme must be normalized")
    .with_scheme("mock")
    .expect("duplicate normalized scheme must be accepted")
    .with_provider_metadata(UserMetadata::new());

    assert_eq!(&["mock"], info.schemes());
    assert!(info.provider_metadata().is_empty());
    assert!(info.clone().with_scheme("1invalid").is_err());
}

#[test]
fn test_with_scheme_rejects_uri_and_path_input() {
    for scheme in [
        "",
        "1file",
        "file:/suffix",
        "file://host",
        "file/path",
        " file",
        "fíle",
        "file?query",
        "file#fragment",
    ] {
        let error = FileSystemInfo::new(
            FileSystemId::new("scheme-test").expect("valid filesystem identity"),
            "mock",
            PathSemantics::Hierarchical,
        )
        .with_scheme(scheme)
        .expect_err("only a complete URI scheme should be accepted");
        assert_eq!(FsErrorKind::InvalidUri, error.kind(), "input: {scheme:?}");
        assert_eq!(FsOperation::ParseUri, error.operation(), "input: {scheme:?}");
    }

    let info = FileSystemInfo::new(
        FileSystemId::new("scheme-valid-test").expect("valid filesystem identity"),
        "mock",
        PathSemantics::Hierarchical,
    )
    .with_scheme("A+B.C-D")
    .expect("valid URI scheme should be accepted")
    .with_scheme("a+b.c-d")
    .expect("duplicate scheme should be accepted");
    assert_eq!(&["a+b.c-d"], info.schemes());
}

#[test]
fn test_file_system_info_rejects_secret_bearing_provider_metadata() {
    assert!(UserMetadata::new().with("access token", "secret").is_err());
}

#[test]
fn test_file_system_info_rejects_sensitive_provider_metadata_keys() {
    assert!(UserMetadata::new().with("x-amz-signature", "plaintext").is_err());
}

#[test]
fn test_file_system_info_preserves_validated_provider_metadata() {
    let metadata = UserMetadata::new()
        .with("provider", "ready")
        .expect("ordinary provider metadata key must be accepted");
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance").expect("valid filesystem identity must be accepted"),
        "mock",
        PathSemantics::ProviderSpecific,
    )
    .with_provider_metadata(metadata);

    assert!(info.provider_metadata().contains_key("provider"));
}

#[test]
fn test_file_system_id_validates_and_displays_provider_identity() {
    let id = FileSystemId::new("tenant-a").expect("valid filesystem identity must be accepted");

    assert_eq!("tenant-a", id.as_str());
    assert_eq!("tenant-a", id.to_string());
    assert!(FileSystemId::new("bad\nidentity").is_err());
}

#[test]
fn test_file_system_info_exposes_snapshot_accessors() {
    let id = FileSystemId::new("file-system-info-test").expect("valid id");
    let info = FileSystemInfo::new(id.clone(), "provider", PathSemantics::Hierarchical)
        .with_scheme("file")
        .expect("valid scheme")
        .with_provider_metadata(UserMetadata::new());

    assert_eq!(info.id(), &id);
    assert_eq!(info.provider_id(), "provider");
    assert_eq!(info.schemes(), &["file"]);
    assert_eq!(info.path_semantics(), PathSemantics::Hierarchical);
    assert!(info.provider_metadata().is_empty());
}
