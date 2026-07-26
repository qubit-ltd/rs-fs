// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{DeleteOptions, FileSystemCapabilities, FileSystemCapability};

#[test]
fn test_delete_options_full_configuration_is_usable() {
    let options = DeleteOptions {
        recursive: true,
        missing_ok: true,
        if_match: Some("v1".to_owned()),
    };

    assert!(options.recursive);
    assert!(options.missing_ok);
    assert_eq!(Some("v1"), options.if_match.as_deref());
}

#[test]
fn delete_requirements_are_checked_against_typed_capabilities() {
    let recursive = DeleteOptions {
        recursive: true,
        ..DeleteOptions::default()
    };
    let error = recursive
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::RecursiveDelete),
        error.required_capability(),
    );

    let conditional = DeleteOptions {
        if_match: Some("v1".to_owned()),
        ..DeleteOptions::default()
    };
    let error = conditional
        .validate_against(FileSystemCapabilities::default())
        .unwrap_err();
    assert_eq!(
        Some(FileSystemCapability::ConditionalDelete),
        error.required_capability(),
    );

    let capabilities = FileSystemCapabilities::default()
        .with(FileSystemCapability::RecursiveDelete)
        .with(FileSystemCapability::ConditionalDelete);
    assert!(recursive.validate_against(capabilities).is_ok());
    assert!(conditional.validate_against(capabilities).is_ok());
}
