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
    FileSystemLimits,
};

#[test]
fn capability_set_reports_typed_guarantees_and_limits() {
    let limits = FileSystemLimits {
        max_path_bytes: Some(4096),
        ..FileSystemLimits::default()
    };
    let capabilities = FileSystemCapabilities::new(limits)
        .with(FileSystemCapability::Read)
        .with(FileSystemCapability::AtomicRename);

    assert!(capabilities.contains(FileSystemCapability::Read));
    assert!(capabilities.contains(FileSystemCapability::AtomicRename));
    assert!(!capabilities.contains(FileSystemCapability::AtomicReplace));
    assert_eq!(Some(4096), capabilities.limits().max_path_bytes);
}

#[test]
fn capability_set_supports_mutation_and_an_empty_default() {
    let mut capabilities = FileSystemCapabilities::default();
    assert!(!capabilities.contains(FileSystemCapability::Write));
    assert_eq!(&FileSystemLimits::default(), capabilities.limits());

    capabilities.insert(FileSystemCapability::Write);
    assert!(capabilities.contains(FileSystemCapability::Write));
}
