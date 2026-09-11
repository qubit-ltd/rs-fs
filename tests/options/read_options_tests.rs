// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::read::ChecksumPolicy;
use qubit_fs::read::ReadOptions;

#[test]
fn test_read_options_full_configuration_is_usable() {
    let options = ReadOptions::default()
        .with_offset(Some(1))
        .with_length(Some(2))
        .with_if_match(Some(ResourceVersion::from("a")))
        .with_if_none_match(Some(ResourceVersion::from("b")))
        .with_checksum(ChecksumPolicy::Required);

    assert_eq!(Some(1), options.offset());
    assert_eq!(Some(2), options.length());
    assert_eq!(Some("a"), options.if_match().map(ResourceVersion::as_str),);
    assert_eq!(Some("b"), options.if_none_match().map(ResourceVersion::as_str),);
    assert_eq!(ChecksumPolicy::Required, options.checksum());
}

#[test]
fn test_read_options_empty_optional_values_are_exposed() {
    let options = ReadOptions::default();

    assert_eq!(None, options.offset());
    assert_eq!(None, options.length());
    assert_eq!(None, options.if_match());
    assert_eq!(None, options.if_none_match());
    assert_eq!(ChecksumPolicy::None, options.checksum());
}

#[test]
fn read_requirements_are_checked_against_typed_capabilities() {
    let conflicting = ReadOptions::default()
        .with_if_match(Some(ResourceVersion::from("v1")))
        .with_if_none_match(Some(ResourceVersion::from("v2")));
    assert_eq!(
        FsErrorKind::InvalidOptions,
        conflicting
            .validate_against(FileSystemCapabilities::default())
            .unwrap_err()
            .kind(),
    );

    let range = ReadOptions::default().with_offset(Some(1));
    let error = range.validate_against(FileSystemCapabilities::default()).unwrap_err();
    assert_eq!(Some(FileSystemCapability::RangeRead), error.required_capability());

    let conditional = ReadOptions::default().with_if_match(Some(ResourceVersion::from("v1")));
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(Some(FileSystemCapability::ConditionalRead), error.required_capability(),);

    let checksummed = ReadOptions::default().with_checksum(ChecksumPolicy::Required);
    let error = checksummed
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ChecksumValidation),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with_guaranteed(FileSystemCapability::RangeRead)
        .with_guaranteed(FileSystemCapability::ConditionalRead)
        .with_guaranteed(FileSystemCapability::ChecksumValidation);
    assert!(
        ReadOptions::default()
            .with_offset(Some(0))
            .with_length(Some(1))
            .with_if_none_match(Some(ResourceVersion::from("v2")))
            .with_checksum(ChecksumPolicy::Required)
            .validate_against(capabilities)
            .is_ok()
    );
}

#[test]
fn read_options_accessors_are_callable_directly() {
    let options = ReadOptions::default()
        .with_offset(Some(3))
        .with_length(Some(4))
        .with_if_match(Some(ResourceVersion::from("match")))
        .with_if_none_match(None)
        .with_checksum(ChecksumPolicy::Required);
    let offset: fn(&ReadOptions) -> Option<u64> = ReadOptions::offset;
    let length: fn(&ReadOptions) -> Option<u64> = ReadOptions::length;
    let if_match: fn(&ReadOptions) -> Option<&ResourceVersion> = ReadOptions::if_match;
    let if_none_match: fn(&ReadOptions) -> Option<&ResourceVersion> = ReadOptions::if_none_match;
    let checksum: fn(&ReadOptions) -> ChecksumPolicy = ReadOptions::checksum;

    assert_eq!(Some(3), offset(&options));
    assert_eq!(Some(4), length(&options));
    assert_eq!(Some("match"), if_match(&options).map(ResourceVersion::as_str));
    assert_eq!(None, if_none_match(&options));
    assert_eq!(ChecksumPolicy::Required, checksum(&options));
}

#[test]
fn read_options_rejects_ranges_past_u64_maximum() {
    let options = ReadOptions::default().with_offset(Some(u64::MAX)).with_length(Some(1));
    assert_eq!(FsErrorKind::InvalidOptions, options.validate().unwrap_err().kind());
}
