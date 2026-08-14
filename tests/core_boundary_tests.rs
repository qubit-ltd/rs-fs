// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! ```compile_fail
//! use qubit_fs::FileResource;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::FsPath;
//! ```
//!
//! ```compile_fail
//! use qubit_fs::FileSystemSpi;
//! ```

use std::hint::black_box;

use qubit_fs::AchievedAtomicity;
#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::ChecksumPolicy;
use qubit_fs::CopyMethod;
use qubit_fs::CopyMode;
use qubit_fs::CopyOptions;
use qubit_fs::CopyOutcome;
use qubit_fs::CopyStats;
use qubit_fs::CreateDirectoryOptions;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOutcome;
use qubit_fs::FileSystem;
use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::MetadataPreservePolicy;
use qubit_fs::NonSensitiveMetadata;
use qubit_fs::Path;
use qubit_fs::PathSemantics;
use qubit_fs::PersistCleanupState;
use qubit_fs::PersistOutcome;
use qubit_fs::PublicationMethod;
use qubit_fs::ReadOptions;
use qubit_fs::ResourceVersion;
use qubit_fs::UserMetadata;
use qubit_fs::WriteOptions;

/// Asserts that a facade can be cloned without requiring it to be copyable.
fn assert_clone<T: Clone>() {}

/// Verifies facade types remain application-level concrete values.
#[test]
fn test_file_system_facades_are_clone() {
    assert_clone::<FileSystem>();
    #[cfg(feature = "async")]
    assert_clone::<AsyncFileSystem>();
}

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

/// Exercises public inline accessors through function pointers so coverage
/// records their externally observable contract directly.
#[test]
fn test_public_value_accessors_preserve_core_contracts() {
    let metadata = UserMetadata::new()
        .with("content-language", "en")
        .expect("a safe metadata key should be accepted");
    let safe = NonSensitiveMetadata::from(metadata.clone());
    let version = ResourceVersion::new("v1");

    let metadata_get: for<'a> fn(&'a UserMetadata, &str) -> Option<&'a str> =
        UserMetadata::get;
    let metadata_contains: fn(&UserMetadata, &str) -> bool =
        UserMetadata::contains_key;
    let safe_get: for<'a> fn(
        &'a NonSensitiveMetadata,
        &str,
    ) -> Option<&'a str> = NonSensitiveMetadata::get;
    let safe_contains: fn(&NonSensitiveMetadata, &str) -> bool =
        NonSensitiveMetadata::contains_key;
    let version_text: for<'a> fn(&'a ResourceVersion) -> &'a str =
        ResourceVersion::as_str;

    assert_eq!(Some("en"), metadata_get(&metadata, "content-language"));
    assert!(metadata_contains(&metadata, "content-language"));
    assert_eq!(Some("en"), safe_get(&safe, "content-language"));
    assert!(safe_contains(&safe, "content-language"));
    assert_eq!("v1", version_text(&version));
}

/// Keeps accessor coverage from depending on compiler inlining decisions.
#[test]
fn test_public_option_and_outcome_accessors_are_stable() {
    let copy_options = CopyOptions::default().with_mode(CopyMode::File);
    let copy_mode: fn(&CopyOptions) -> CopyMode = black_box(CopyOptions::mode);
    assert_eq!(CopyMode::File, copy_mode(&copy_options));

    let copy_outcome = CopyOutcome::new(
        CopyStats::default(),
        CopyMethod::Native,
        AchievedAtomicity::Atomic,
    )
    .with_metadata(MetadataPreservePolicy::Portable)
    .with_target_version(ResourceVersion::new("v2"))
    .with_durable(true);
    let durable: fn(&CopyOutcome) -> bool = black_box(CopyOutcome::durable);
    let target_version: fn(&CopyOutcome) -> Option<&ResourceVersion> =
        black_box(CopyOutcome::target_version);
    let used_fallback: fn(&CopyOutcome) -> bool =
        black_box(CopyOutcome::used_fallback);
    assert!(durable(&copy_outcome));
    assert_eq!(
        Some("v2"),
        target_version(&copy_outcome).map(ResourceVersion::as_str)
    );
    assert!(!used_fallback(&copy_outcome));

    let create_options = CreateDirectoryOptions::default()
        .with_recursive(true)
        .with_exists_ok(true);
    let recursive: fn(&CreateDirectoryOptions) -> bool =
        black_box(CreateDirectoryOptions::recursive);
    let exists_ok: fn(&CreateDirectoryOptions) -> bool =
        black_box(CreateDirectoryOptions::exists_ok);
    let user_metadata =
        black_box(CreateDirectoryOptions::user_metadata)(&create_options);
    assert!(recursive(&create_options));
    assert!(exists_ok(&create_options));
    assert!(user_metadata.is_empty());

    let created = CreateDirectoryOutcome::new(false).with_created_ancestors(2);
    let ancestors: fn(CreateDirectoryOutcome) -> Option<u64> =
        black_box(CreateDirectoryOutcome::created_ancestors);
    assert_eq!(Some(2), ancestors(created));

    let deleted = DeleteOutcome::new(false).with_deleted_entries(2);
    let deleted_entries: fn(DeleteOutcome) -> Option<u64> =
        black_box(DeleteOutcome::deleted_entries);
    assert_eq!(Some(2), deleted_entries(deleted));

    let read_options = ReadOptions::default();
    let offset: fn(&ReadOptions) -> Option<u64> =
        black_box(ReadOptions::offset);
    let checksum: fn(&ReadOptions) -> ChecksumPolicy =
        black_box(ReadOptions::checksum);
    assert_eq!(None, offset(&read_options));
    assert_eq!(ChecksumPolicy::None, checksum(&read_options));

    let write_options = WriteOptions::default().with_create_parent(true);
    let create_parent: fn(&WriteOptions) -> bool =
        black_box(WriteOptions::create_parent);
    assert!(create_parent(&write_options));

    let persist = PersistOutcome::new(
        Path::root(),
        AchievedAtomicity::Atomic,
        PublicationMethod::Direct,
    )
    .with_cleanup_state(PersistCleanupState::Complete);
    let cleanup_state: fn(&PersistOutcome) -> PersistCleanupState =
        black_box(PersistOutcome::cleanup_state);
    assert_eq!(PersistCleanupState::Complete, cleanup_state(&persist));
}
