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
    MetadataPreservePolicy,
    PersistOptions,
};

#[test]
fn test_persist_options_full_configuration_is_usable() {
    let options = PersistOptions {
        overwrite: true,
        atomicity: AtomicityRequirement::Preferred,
        preserve_metadata: MetadataPreservePolicy::All,
    };

    assert!(options.overwrite);
    assert_eq!(AtomicityRequirement::Preferred, options.atomicity);
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
