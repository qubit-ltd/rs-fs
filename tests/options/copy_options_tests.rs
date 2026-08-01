// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    AtomicityRequirement,
    CopyConflictPolicy,
    CopyMode,
    CopyOptions,
    DurabilityRequirement,
    FileSystemCapabilities,
    FileSystemCapability,
    MetadataPreservePolicy,
    ServerSidePreference,
};

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
                    .with(FileSystemCapability::ServerSideCopy),
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
        .with_follow_symlinks(true)
        .with_create_parent(true)
        .with_continue_on_error(true)
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Required);

    assert_eq!(CopyMode::Tree, options.mode());
    assert_eq!(CopyConflictPolicy::Skip, options.conflict());
    assert!(options.follow_symlinks());
    assert!(options.create_parent());
    assert!(options.continue_on_error());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert_eq!(DurabilityRequirement::Required, options.durability());
}
