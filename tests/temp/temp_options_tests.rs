// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Coverage for evolution-safe temporary-resource option builders.

use qubit_fs::{
    Path,
    TempDirectoryOptions,
    TempFileOptions,
};

#[test]
fn temp_file_options_expose_builder_and_getter_api() {
    let parent = Path::parse("/tmp").expect("parent path should parse");
    let options = TempFileOptions::default()
        .with_parent(Some(parent.clone()))
        .with_prefix("prefix-".to_owned())
        .with_suffix(".data".to_owned());

    assert_eq!(Some(&parent), options.parent());
    assert_eq!("prefix-", options.prefix());
    assert_eq!(".data", options.suffix());
}

#[test]
fn temp_directory_options_expose_builder_and_getter_api() {
    let parent = Path::parse("/tmp").expect("parent path should parse");
    let options = TempDirectoryOptions::default()
        .with_parent(Some(parent.clone()))
        .with_prefix("dir-".to_owned())
        .with_suffix("-suffix".to_owned());

    assert_eq!(Some(&parent), options.parent());
    assert_eq!("dir-", options.prefix());
    assert_eq!("-suffix", options.suffix());
}
