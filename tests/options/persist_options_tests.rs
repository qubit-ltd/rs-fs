// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::AtomicityRequirement;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::FsErrorKind;
use qubit_fs::MetadataPreservePolicy;
use qubit_fs::PersistOptions;

#[test]
fn test_persist_options_full_configuration_is_usable() {
    let options = PersistOptions::default()
        .with_overwrite(true)
        .with_atomicity(AtomicityRequirement::Preferred)
        .with_preserve_metadata(MetadataPreservePolicy::All);

    assert!(options.overwrite());
    assert_eq!(AtomicityRequirement::Preferred, options.atomicity());
    assert_eq!(MetadataPreservePolicy::All, options.preserve_metadata());
}

#[test]
fn required_persist_atomicity_fails_preflight_without_atomic_temp_capability() {
    let options = PersistOptions::default();

    let error = options
        .validate_against(FileSystemCapabilities::default())
        .expect_err("required atomic persist should fail preflight");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert_eq!(
        Some(FileSystemCapability::AtomicTempPersist),
        error.required_capability(),
    );
}
