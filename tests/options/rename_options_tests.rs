// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    AtomicityRequirement,
    FileSystemCapabilities,
    FileSystemCapability,
    FsErrorKind,
    RenameOptions,
};

#[test]
fn test_rename_options_full_configuration_and_default_are_usable() {
    let options = RenameOptions::default()
        .with_overwrite(true)
        .with_atomicity(AtomicityRequirement::Required);

    assert!(options.overwrite());
    assert_eq!(AtomicityRequirement::Required, options.atomicity());
    assert!(!RenameOptions::default().overwrite());
    assert_eq!(
        AtomicityRequirement::Preferred,
        RenameOptions::default().atomicity(),
    );
}

#[test]
fn required_rename_atomicity_fails_preflight_without_side_effects() {
    let options = RenameOptions::default()
        .with_atomicity(AtomicityRequirement::Required);

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
                    .with(FileSystemCapability::AtomicRename),
            )
            .is_ok()
    );
}
