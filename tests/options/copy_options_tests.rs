// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::copy::CopyConflictPolicy;
use qubit_fs::copy::CopyMode;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::MetadataPreservePolicy;
use qubit_fs::copy::ServerSidePreference;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::DurabilityRequirement;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::SymlinkPolicy;

#[test]
fn test_copy_options_default_and_constructors_set_modes() {
    assert_eq!(CopyMode::Auto, CopyOptions::default().mode());
    assert_eq!(CopyMode::File, CopyOptions::file().mode());
    assert_eq!(CopyMode::Tree, CopyOptions::tree().mode());
}

#[test]
fn required_server_side_copy_is_checked_before_side_effects() {
    let options =
        CopyOptions::default().with_server_side(ServerSidePreference::Require);
    let error = options
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ServerSideCopy),
        error.required_capability(),
    );
    assert!(
        options
            .validate_against(
                FileSystemCapabilities::default()
                    .with_guaranteed(FileSystemCapability::ServerSideCopy),
            )
            .is_ok()
    );
}

#[test]
fn test_copy_options_full_configuration_is_usable() {
    let options = CopyOptions::default()
        .with_mode(CopyMode::Tree)
        .with_conflict(CopyConflictPolicy::Skip)
        .with_preserve_metadata(MetadataPreservePolicy::ProviderNative)
        .with_server_side(ServerSidePreference::Disable)
        .with_symlink_policy(SymlinkPolicy::FollowWithinFileSystem)
        .with_create_parent(true)
        .with_continue_on_error(true)
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Required);

    assert_eq!(CopyMode::Tree, options.mode());
    assert_eq!(CopyConflictPolicy::Skip, options.conflict());
    assert_eq!(
        Some(SymlinkPolicy::FollowWithinFileSystem),
        options.symlink_policy_override(),
    );
    assert!(options.create_parent());
    assert!(options.continue_on_error());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert_eq!(DurabilityRequirement::Required, options.durability());
}

#[test]
fn test_copy_options_validate_guarantees_for_explicit_source_mode() {
    let file = CopyOptions::file()
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Required);
    let file_capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::AtomicFileCopy)
        .with_guaranteed(FileSystemCapability::DurableFileCopy);
    assert!(file.validate_against(file_capabilities).is_ok());
    let tree_error = CopyOptions::tree()
        .with_atomicity(AtomicityRequirement::Required)
        .validate_against(file_capabilities)
        .expect_err("file atomicity must not imply tree atomicity");
    assert_eq!(
        Some(FileSystemCapability::AtomicTreeCopy),
        tree_error.required_capability(),
    );
}

#[test]
fn test_copy_options_auto_defers_when_one_source_kind_is_supported() {
    let options = CopyOptions::default()
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Required);
    let file_capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::AtomicFileCopy)
        .with_guaranteed(FileSystemCapability::DurableFileCopy);
    assert!(options.validate_against(file_capabilities).is_ok());

    let error = options
        .validate_against(FileSystemCapabilities::new())
        .expect_err("Auto must fail when no source kind has atomic support");
    assert_eq!(
        Some(FileSystemCapability::AtomicFileCopy),
        error.required_capability(),
    );
}
