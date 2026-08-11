// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::DeleteOptions;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::ResourceVersion;

#[test]
fn test_delete_options_full_configuration_is_usable() {
    let options = DeleteOptions::default()
        .with_recursive(true)
        .with_missing_ok(true)
        .with_if_match(Some(ResourceVersion::from("v1")));

    assert!(options.recursive());
    assert!(options.missing_ok());
    assert_eq!(Some("v1"), options.if_match().map(ResourceVersion::as_str),);
}

#[test]
fn delete_requirements_are_checked_against_typed_capabilities() {
    let recursive = DeleteOptions::default().with_recursive(true);
    let error = recursive
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::RecursiveDelete),
        error.required_capability(),
    );

    let conditional = DeleteOptions::default()
        .with_if_match(Some(ResourceVersion::from("v1")));
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ConditionalDelete),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with_guaranteed(FileSystemCapability::RecursiveDelete)
        .with_guaranteed(FileSystemCapability::ConditionalDelete);
    assert!(recursive.validate_against(capabilities).is_ok());
    assert!(conditional.validate_against(capabilities).is_ok());
}
