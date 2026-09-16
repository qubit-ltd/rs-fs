// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Coverage for evolution-safe temporary-resource option builders.

use qubit_fs::Path;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::TempOptions;

#[test]
fn one_temp_options_type_configures_both_resource_kinds() {
    let parent = Path::parse("/tmp").expect("parent path should parse");
    let options = TempOptions::new()
        .with_parent(Some(parent.clone()))
        .with_prefix("report-")
        .with_suffix(".tmp")
        .with_create_parent(true);

    assert_eq!(Some(&parent), options.parent());
    assert_eq!("report-", options.prefix());
    assert_eq!(".tmp", options.suffix());
    assert!(options.creates_parent());
}

#[test]
fn temp_options_accessors_are_callable_directly() {
    let parent = Path::parse("tmp").expect("parent path should parse");
    let options = TempOptions::default()
        .with_parent(Some(parent.clone()))
        .with_prefix("prefix")
        .with_suffix("suffix")
        .with_create_parent(true);
    let parent_accessor: fn(&TempOptions) -> Option<&Path> = TempOptions::parent;
    let prefix: fn(&TempOptions) -> &str = TempOptions::prefix;
    let suffix: fn(&TempOptions) -> &str = TempOptions::suffix;
    let creates_parent: fn(&TempOptions) -> bool = TempOptions::creates_parent;

    assert_eq!(Some(&parent), parent_accessor(&options));
    assert_eq!("prefix", prefix(&options));
    assert_eq!("suffix", suffix(&options));
    assert!(creates_parent(&options));
}

#[test]
fn persist_options_expose_parent_creation_policy() {
    assert!(!PersistOptions::default().creates_parent());
    assert!(PersistOptions::default().with_create_parent().creates_parent());
}
