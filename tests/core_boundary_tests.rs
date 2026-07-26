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
    UserMetadata,
};

/// Verifies core provider identity is represented without an SPI wrapper.
#[test]
fn test_file_system_info_stores_provider_as_text() {
    let info = FileSystemInfo::new(
        FileSystemId::new("local-instance")
            .expect("the filesystem ID should validate"),
        "local",
        PathSemantics::Hierarchical,
    );
    assert_eq!("local", info.provider_id());
}

/// Verifies provider-neutral metadata is an ordered string map.
#[test]
fn test_user_metadata_is_provider_neutral() {
    let metadata = UserMetadata::new()
        .with("content-language", "en")
        .expect("a safe metadata key should be accepted");
    assert_eq!(Some("en"), metadata.get("content-language"));
    assert!(format!("{metadata:?}").contains("content-language"));
    assert!(!format!("{metadata:?}").contains("\"en\""));
}
