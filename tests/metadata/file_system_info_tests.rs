// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::PathSemantics;
use qubit_fs::UserMetadata;

#[test]
fn file_system_info_is_a_validated_local_snapshot() {
    let id = FileSystemId::new("mock-instance")
        .expect("valid filesystem identity must be accepted");
    let provider_id = "mock";
    let info =
        FileSystemInfo::new(id.clone(), provider_id, PathSemantics::ObjectKey)
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
fn file_system_info_deduplicates_schemes_and_replaces_provider_metadata() {
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance")
            .expect("valid filesystem identity must be accepted"),
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
fn file_system_info_rejects_secret_bearing_provider_metadata() {
    assert!(UserMetadata::new().with("access token", "secret").is_err());
}

#[test]
fn file_system_info_rejects_sensitive_provider_metadata_keys() {
    assert!(
        UserMetadata::new()
            .with("x-amz-signature", "plaintext")
            .is_err()
    );
}

#[test]
fn file_system_info_preserves_validated_provider_metadata() {
    let metadata = UserMetadata::new()
        .with("provider", "ready")
        .expect("ordinary provider metadata key must be accepted");
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance")
            .expect("valid filesystem identity must be accepted"),
        "mock",
        PathSemantics::ProviderSpecific,
    )
    .with_provider_metadata(metadata);

    assert!(info.provider_metadata().contains_key("provider"));
}

#[test]
fn file_system_id_validates_and_displays_provider_identity() {
    let id = FileSystemId::new("tenant-a")
        .expect("valid filesystem identity must be accepted");

    assert_eq!("tenant-a", id.as_str());
    assert_eq!("tenant-a", id.to_string());
    assert!(FileSystemId::new("bad\nidentity").is_err());
}
