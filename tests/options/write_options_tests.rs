// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::AtomicityRequirement;
use qubit_fs::Checksum;
use qubit_fs::ChecksumAlgorithm;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::FsErrorKind;
use qubit_fs::ResourceVersion;
use qubit_fs::UserMetadata;
use qubit_fs::WriteDisposition;
use qubit_fs::WriteOptions;
use qubit_fs::WritePrecondition;

#[test]
fn test_write_options_full_configuration_is_usable() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    let options = WriteOptions::default()
        .with_create_parent(true)
        .with_disposition(WriteDisposition::CreateOrReplace)
        .with_atomicity(AtomicityRequirement::Required)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new(
            "v1",
        )))
        .with_content_type(Some("text/plain".to_owned()))
        .with_checksum(Some(checksum));

    assert!(options.create_parent());
    assert_eq!(Some("text/plain"), options.content_type());
    assert!(options.checksum().is_some());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert!(options.validate().is_ok());
}

#[test]
fn write_requirements_are_checked_against_typed_capabilities() {
    let atomic =
        WriteOptions::default().with_atomicity(AtomicityRequirement::Required);
    let error = atomic
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::AtomicReplace),
        error.required_capability()
    );

    let append = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::NotRequired);
    let error = append
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::Append),
        error.required_capability()
    );

    let conditional =
        WriteOptions::default().with_precondition(WritePrecondition::IfAbsent);
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ConditionalWrite),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with_guaranteed(FileSystemCapability::AtomicReplace)
        .with_guaranteed(FileSystemCapability::Append)
        .with_guaranteed(FileSystemCapability::ConditionalWrite);
    assert!(atomic.validate_against(capabilities).is_ok());
    assert!(append.validate_against(capabilities).is_ok());
    assert!(conditional.validate_against(capabilities).is_ok());
}

#[test]
fn append_cannot_request_atomic_publication() {
    let options = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::Required);

    let error = options
        .validate()
        .expect_err("append plus required publication atomicity is invalid");
    assert_eq!(qubit_fs::FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn append_rejects_version_preconditions_but_allows_plain_append() {
    let invalid = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new(
            "v1",
        )));
    assert!(invalid.validate().is_err());

    let valid = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::NotRequired);
    assert!(valid.validate().is_ok());
}

#[test]
fn create_new_rejects_if_match_precondition() {
    let options = WriteOptions::default()
        .with_disposition(WriteDisposition::CreateNew)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new(
            "v1",
        )));

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
    assert!(options.user_metadata().contains_key("category"));
    assert!(!format!("{options:?}").contains("private-category"));
}
