// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    AtomicityRequirement,
    Checksum,
    ChecksumAlgorithm,
    FileSystemCapabilities,
    FileSystemCapability,
    FsErrorKind,
    NonSensitiveMetadata,
    ResourceVersion,
    UserMetadata,
    WriteDisposition,
    WriteOptions,
    WritePrecondition,
};

#[test]
fn test_write_options_full_configuration_is_usable() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    let options = WriteOptions {
        create_parent: true,
        disposition: WriteDisposition::CreateOrReplace,
        atomicity: AtomicityRequirement::Required,
        precondition: WritePrecondition::IfMatch(ResourceVersion::new("v1")),
        content_type: Some("text/plain".to_owned()),
        user_metadata: NonSensitiveMetadata::new(),
        checksum: Some(checksum),
    };

    assert!(options.create_parent);
    assert_eq!(Some("text/plain"), options.content_type.as_deref());
    assert!(options.checksum.is_some());
    assert_eq!(AtomicityRequirement::Required, options.atomicity);
    assert!(options.validate().is_ok());
}

#[test]
fn write_requirements_are_checked_against_typed_capabilities() {
    let atomic = WriteOptions {
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    let error = atomic
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::AtomicReplace),
        error.required_capability()
    );

    let append = WriteOptions {
        disposition: WriteDisposition::Append,
        atomicity: AtomicityRequirement::NotRequired,
        ..WriteOptions::default()
    };
    let error = append
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::Append),
        error.required_capability()
    );

    let conditional = WriteOptions {
        precondition: WritePrecondition::IfAbsent,
        ..WriteOptions::default()
    };
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ConditionalWrite),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with(FileSystemCapability::AtomicReplace)
        .with(FileSystemCapability::Append)
        .with(FileSystemCapability::ConditionalWrite);
    assert!(atomic.validate_against(capabilities).is_ok());
    assert!(append.validate_against(capabilities).is_ok());
    assert!(conditional.validate_against(capabilities).is_ok());
}

#[test]
fn append_cannot_request_atomic_publication() {
    let options = WriteOptions {
        disposition: WriteDisposition::Append,
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };

    let error = options
        .validate()
        .expect_err("append plus required publication atomicity is invalid");
    assert_eq!(qubit_fs::FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn append_rejects_version_preconditions_but_allows_plain_append() {
    let invalid = WriteOptions {
        disposition: WriteDisposition::Append,
        precondition: WritePrecondition::IfMatch(ResourceVersion::new("v1")),
        ..WriteOptions::default()
    };
    assert!(invalid.validate().is_err());

    let valid = WriteOptions {
        disposition: WriteDisposition::Append,
        atomicity: AtomicityRequirement::NotRequired,
        ..WriteOptions::default()
    };
    assert!(valid.validate().is_ok());
}

#[test]
fn create_new_rejects_if_match_precondition() {
    let options = WriteOptions {
        disposition: WriteDisposition::CreateNew,
        precondition: WritePrecondition::IfMatch(ResourceVersion::new("v1")),
        ..WriteOptions::default()
    };

    let error = options
        .validate()
        .expect_err("CreateNew and IfMatch cannot both be satisfied");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn write_options_preserve_validated_user_metadata() {
    let options = WriteOptions::default().with_user_metadata(
        UserMetadata::new()
            .with("category", "private-category")
            .expect("ordinary user metadata key must be accepted"),
    );
    assert!(options.user_metadata.contains_key("category"));
    assert!(!format!("{options:?}").contains("private-category"));
}
