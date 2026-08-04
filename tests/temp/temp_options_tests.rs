// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for evolution-safe temporary-resource option builders.

use qubit_fs::{
    Path,
    PersistOptions,
    TempDirectoryOptions,
    TempFileOptions,
};

#[test]
fn temp_file_options_expose_builder_and_getter_api() {
    let parent = Path::parse("/tmp").expect("parent path should parse");
    let options = TempFileOptions::default()
        .with_parent(Some(parent.clone()))
        .with_prefix("prefix-".to_owned())
        .with_suffix(".data".to_owned())
        .with_create_parent();

    assert_eq!(Some(&parent), options.parent());
    assert_eq!("prefix-", options.prefix());
    assert_eq!(".data", options.suffix());
    assert!(options.creates_parent());
}

#[test]
fn temp_directory_options_expose_builder_and_getter_api() {
    let parent = Path::parse("/tmp").expect("parent path should parse");
    let options = TempDirectoryOptions::default()
        .with_parent(Some(parent.clone()))
        .with_prefix("dir-".to_owned())
        .with_suffix("-suffix".to_owned())
        .with_create_parent();

    assert_eq!(Some(&parent), options.parent());
    assert_eq!("dir-", options.prefix());
    assert_eq!("-suffix", options.suffix());
    assert!(options.creates_parent());
}

#[test]
fn persist_options_expose_parent_creation_policy() {
    assert!(!PersistOptions::default().creates_parent());
    assert!(
        PersistOptions::default()
            .with_create_parent()
            .creates_parent()
    );
}
