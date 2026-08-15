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
fn persist_options_expose_parent_creation_policy() {
    assert!(!PersistOptions::default().creates_parent());
    assert!(
        PersistOptions::default()
            .with_create_parent()
            .creates_parent()
    );
}
