// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::metadata::DurabilityRequirement;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::rename::RenameOptions;

#[test]
fn test_rename_options_full_configuration_and_default_are_usable() {
    let options = RenameOptions::default()
        .with_overwrite(true)
        .with_atomicity(AtomicityRequirement::Required)
        .with_durability(DurabilityRequirement::Preferred);

    assert!(options.overwrite());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert_eq!(DurabilityRequirement::Preferred, options.durability());
    assert!(!RenameOptions::default().overwrite());
    assert_eq!(
        AtomicityRequirement::Preferred,
        RenameOptions::default().atomicity(),
    );
    assert_eq!(
        DurabilityRequirement::NotRequired,
        RenameOptions::default().durability(),
    );
}

#[test]
fn required_rename_atomicity_fails_preflight_without_side_effects() {
    let options =
        RenameOptions::default().with_atomicity(AtomicityRequirement::Required);

    let error = options
        .validate_against(FileSystemCapabilities::default())
        .expect_err("missing atomic rename guarantee should fail");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert_eq!(
        Some(FileSystemCapability::AtomicRename),
        error.required_capability(),
    );
    assert!(
        options
            .validate_against(
                FileSystemCapabilities::default()
                    .with_guaranteed(FileSystemCapability::AtomicRename),
            )
            .is_ok()
    );
    assert!(
        options
            .validate_against(
                FileSystemCapabilities::default()
                    .with_conditional(FileSystemCapability::AtomicRename),
            )
            .is_ok()
    );
}

#[test]
fn required_rename_durability_fails_without_provider_guarantee() {
    let options = RenameOptions::default()
        .with_durability(DurabilityRequirement::Required);

    let error = options
        .validate_against(FileSystemCapabilities::default())
        .expect_err("missing durable rename guarantee should fail");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert_eq!(
        Some(FileSystemCapability::DurableRename),
        error.required_capability(),
    );
    assert!(
        options
            .validate_against(
                FileSystemCapabilities::default()
                    .with_guaranteed(FileSystemCapability::DurableRename),
            )
            .is_ok()
    );
}
