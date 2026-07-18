// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemId,
    FileSystemInfo,
    PathSemantics,
};
use qubit_spi::ProviderId;

#[test]
fn file_system_info_is_a_validated_local_snapshot() {
    let id = FileSystemId::new("mock-instance").unwrap();
    let provider_id = ProviderId::new("mock").unwrap();
    let info = FileSystemInfo::new(
        id.clone(),
        provider_id.clone(),
        PathSemantics::ObjectKey,
    )
    .with_scheme("mock")
    .unwrap();

    assert_eq!(&id, info.id());
    assert_eq!(&provider_id, info.provider_id());
    assert_eq!(&["mock"], info.schemes());
    assert_eq!(PathSemantics::ObjectKey, info.path_semantics());
    assert!(info.provider_metadata().is_empty());
    assert!(FileSystemId::new("").is_err());
}

#[test]
fn file_system_info_deduplicates_schemes_and_replaces_provider_metadata() {
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance").unwrap(),
        ProviderId::new("mock").unwrap(),
        PathSemantics::ProviderSpecific,
    )
    .with_scheme("MOCK")
    .unwrap()
    .with_scheme("mock")
    .unwrap()
    .with_provider_metadata(qubit_metadata::Metadata::new())
    .unwrap();

    assert_eq!(&["mock"], info.schemes());
    assert!(info.provider_metadata().is_empty());
    assert!(info.clone().with_scheme("1invalid").is_err());
}

#[test]
fn file_system_info_rejects_secret_bearing_provider_metadata() {
    let metadata = qubit_metadata::Metadata::new()
        .with("access token", "secret".to_owned());
    let error = FileSystemInfo::new(
        FileSystemId::new("mock-instance").unwrap(),
        ProviderId::new("mock").unwrap(),
        PathSemantics::ProviderSpecific,
    )
    .with_provider_metadata(metadata)
    .expect_err("provider snapshots must not retain credential material");

    assert_eq!(qubit_fs::FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn file_system_info_rejects_secret_keys_nested_in_json_metadata() {
    let metadata = qubit_metadata::Metadata::new().with(
        "provider",
        serde_json::json!({
            "diagnostics": [
                {"status": "failed"},
                {"x-amz-signature": "plaintext"}
            ]
        }),
    );
    let error = FileSystemInfo::new(
        FileSystemId::new("mock-instance").unwrap(),
        ProviderId::new("mock").unwrap(),
        PathSemantics::ProviderSpecific,
    )
    .with_provider_metadata(metadata)
    .expect_err("debug-visible metadata must reject nested secret keys");

    assert_eq!(qubit_fs::FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn file_system_info_accepts_non_sensitive_nested_json_metadata() {
    let metadata = qubit_metadata::Metadata::new().with(
        "provider",
        serde_json::json!({
            "diagnostics": [
                {"status": "ready"},
                {"region": "primary"}
            ]
        }),
    );
    let info = FileSystemInfo::new(
        FileSystemId::new("mock-instance").unwrap(),
        ProviderId::new("mock").unwrap(),
        PathSemantics::ProviderSpecific,
    )
    .with_provider_metadata(metadata)
    .expect("non-sensitive nested metadata should be accepted");

    assert!(info.provider_metadata().contains_key("provider"));
}

#[test]
fn file_system_id_validates_and_displays_provider_identity() {
    let id = FileSystemId::new("tenant-a").unwrap();

    assert_eq!("tenant-a", id.as_str());
    assert_eq!("tenant-a", id.to_string());
    assert!(FileSystemId::new("bad\nidentity").is_err());
}
