// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::hint::black_box;

use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::Checksum;
use qubit_fs::metadata::ChecksumAlgorithm;
use qubit_fs::metadata::DurabilityRequirement;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::NonSensitiveMetadata;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::metadata::UserMetadata;
use qubit_fs::write::WriteDisposition;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WritePrecondition;

#[test]
fn test_write_options_full_configuration_is_usable() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    let options = WriteOptions::default()
        .with_create_parent(true)
        .with_disposition(WriteDisposition::CreateOrReplace)
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Preferred)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v1")))
        .with_content_type(Some("text/plain".to_owned()))
        .with_checksum(Some(checksum));

    assert!(options.create_parent());
    assert_eq!(Some("text/plain"), options.content_type());
    assert!(options.checksum().is_some());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert_eq!(DurabilityRequirement::Preferred, options.durability());
    assert_eq!(WriteDisposition::CreateOrReplace, options.disposition());
    assert!(matches!(options.precondition(), WritePrecondition::IfMatch(_)));
    assert!(options.user_metadata().is_empty());
    assert!(options.validate().is_ok());
}

#[test]
fn test_write_requirements_are_checked_against_typed_capabilities() {
    let atomic = WriteOptions::default().with_atomicity(AtomicityRequirement::Required);
    let error = atomic.validate_against(FileSystemCapabilities::default()).unwrap_err();
    assert_eq!(Some(FileSystemCapability::AtomicReplace), error.required_capability());

    let append = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::NotRequired);
    let error = append.validate_against(FileSystemCapabilities::default()).unwrap_err();
    assert_eq!(Some(FileSystemCapability::Append), error.required_capability());

    let conditional = WriteOptions::default().with_precondition(WritePrecondition::IfAbsent);
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

    let durable = WriteOptions::default().with_durability(DurabilityRequirement::Required);
    let error = durable
        .validate_against(FileSystemCapabilities::default())
        .expect_err("required durability must be preflighted");
    assert_eq!(Some(FileSystemCapability::DurableWrite), error.required_capability());
    let capabilities = capabilities.with_guaranteed(FileSystemCapability::DurableWrite);
    assert!(durable.validate_against(capabilities).is_ok());
}

#[test]
fn test_append_cannot_request_atomic_publication() {
    let options = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::Required);

    let error = options
        .validate()
        .expect_err("append plus required publication atomicity is invalid");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn test_append_rejects_version_preconditions_but_allows_plain_append() {
    let invalid = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v1")));
    assert!(invalid.validate().is_err());

    let valid = WriteOptions::default()
        .with_disposition(WriteDisposition::Append)
        .with_atomicity(AtomicityRequirement::NotRequired);
    assert!(valid.validate().is_ok());
}

#[test]
fn test_create_new_rejects_if_match_precondition() {
    let options = WriteOptions::default()
        .with_disposition(WriteDisposition::CreateNew)
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v1")));

    let error = options
        .validate()
        .expect_err("CreateNew and IfMatch cannot both be satisfied");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn test_write_options_preserve_validated_user_metadata() {
    let options = WriteOptions::default().with_user_metadata(
        UserMetadata::new()
            .with("category", "private-category")
            .expect("ordinary user metadata key must be accepted"),
    );
    assert!(options.user_metadata().contains_key("category"));
    assert!(!format!("{options:?}").contains("private-category"));
}

#[test]
fn test_option_accessors_are_executed_at_runtime() {
    let disposition: fn(&WriteOptions) -> WriteDisposition = black_box(WriteOptions::disposition);
    let precondition: for<'a> fn(&'a WriteOptions) -> &'a WritePrecondition = black_box(WriteOptions::precondition);
    let create_parent: fn(&WriteOptions) -> bool = black_box(WriteOptions::create_parent);
    let user_metadata: fn(&WriteOptions) -> &NonSensitiveMetadata = black_box(WriteOptions::user_metadata);
    let checksum: for<'a> fn(&'a WriteOptions) -> Option<&'a Checksum> = black_box(WriteOptions::checksum);

    let options = WriteOptions::default()
        .with_create_parent(true)
        .with_disposition(WriteDisposition::CreateOrReplace)
        .with_checksum(Some(Checksum::new(ChecksumAlgorithm::Sha256, "abc")));

    assert_eq!(WriteDisposition::CreateOrReplace, disposition(&options));
    assert!(matches!(precondition(&options), WritePrecondition::None));
    assert!(create_parent(&options));
    assert!(user_metadata(&options).is_empty());
    assert!(checksum(&options).is_some());
}
