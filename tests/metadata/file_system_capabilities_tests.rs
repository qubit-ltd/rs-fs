// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileSystemCapabilities,
    FileSystemCapability,
};

#[test]
fn capability_set_reports_typed_guarantees() {
    let capabilities = FileSystemCapabilities::new()
        .with(FileSystemCapability::Read)
        .with(FileSystemCapability::AtomicRename);

    assert!(capabilities.contains(FileSystemCapability::Read));
    assert!(capabilities.contains(FileSystemCapability::AtomicRename));
    assert!(!capabilities.contains(FileSystemCapability::AtomicReplace));
}

#[test]
fn capability_set_supports_mutation_and_an_empty_default() {
    let mut capabilities = FileSystemCapabilities::default();
    assert!(!capabilities.contains(FileSystemCapability::Write));

    capabilities.insert(FileSystemCapability::Write);
    assert!(capabilities.contains(FileSystemCapability::Write));
}
