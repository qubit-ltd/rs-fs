// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    ChecksumPolicy,
    FileSystemCapabilities,
    FileSystemCapability,
    ReadOptions,
};

#[test]
fn test_read_options_full_configuration_is_usable() {
    let options = ReadOptions {
        offset: Some(1),
        length: Some(2),
        if_match: Some("a".to_owned()),
        if_none_match: Some("b".to_owned()),
        checksum: ChecksumPolicy::Required,
    };

    assert_eq!(Some(1), options.offset);
    assert_eq!(Some(2), options.length);
    assert_eq!(Some("a"), options.if_match.as_deref());
    assert_eq!(Some("b"), options.if_none_match.as_deref());
    assert_eq!(ChecksumPolicy::Required, options.checksum);
}

#[test]
fn read_requirements_are_checked_against_typed_capabilities() {
    let conflicting = ReadOptions {
        if_match: Some("v1".to_owned()),
        if_none_match: Some("v2".to_owned()),
        ..ReadOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::InvalidOptions,
        conflicting
            .validate_against(FileSystemCapabilities::default())
            .unwrap_err()
            .kind(),
    );

    let range = ReadOptions {
        offset: Some(1),
        ..ReadOptions::default()
    };
    let error = range
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::RangeRead),
        error.required_capability()
    );

    let conditional = ReadOptions {
        if_match: Some("v1".to_owned()),
        ..ReadOptions::default()
    };
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ConditionalRead),
        error.required_capability(),
    );

    let checksummed = ReadOptions {
        checksum: ChecksumPolicy::Required,
        ..ReadOptions::default()
    };
    let error = checksummed
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ChecksumValidation),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with(FileSystemCapability::RangeRead)
        .with(FileSystemCapability::ConditionalRead)
        .with(FileSystemCapability::ChecksumValidation);
    assert!(
        ReadOptions {
            offset: Some(0),
            length: Some(1),
            if_none_match: Some("v2".to_owned()),
            checksum: ChecksumPolicy::Required,
            ..ReadOptions::default()
        }
        .validate_against(capabilities)
        .is_ok()
    );
}
