// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{FileSystemCapabilities, FileSystemCapability};

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

#[test]
fn capability_discriminants_remain_stable_for_capability_sets() {
    let capabilities = [
        FileSystemCapability::List,
        FileSystemCapability::Read,
        FileSystemCapability::RangeRead,
        FileSystemCapability::ConditionalRead,
        FileSystemCapability::ChecksumValidation,
        FileSystemCapability::Write,
        FileSystemCapability::Append,
        FileSystemCapability::ConditionalWrite,
        FileSystemCapability::CreateDirectory,
        FileSystemCapability::EmptyDirectory,
        FileSystemCapability::Delete,
        FileSystemCapability::RecursiveDelete,
        FileSystemCapability::ConditionalDelete,
        FileSystemCapability::Rename,
        FileSystemCapability::AtomicRename,
        FileSystemCapability::AtomicReplace,
        FileSystemCapability::Copy,
        FileSystemCapability::ServerSideCopy,
        FileSystemCapability::Symlink,
        FileSystemCapability::TempFile,
        FileSystemCapability::TempDirectory,
        FileSystemCapability::AtomicTempPersist,
    ];

    for (expected, capability) in capabilities.into_iter().enumerate() {
        assert_eq!(expected as u8, capability as u8);
    }
}
