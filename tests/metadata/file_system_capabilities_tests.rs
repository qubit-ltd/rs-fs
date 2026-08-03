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
    assert!(capabilities.is_empty());
    assert_eq!(0, capabilities.len());
    assert!(!capabilities.contains(FileSystemCapability::Write));

    capabilities.insert(FileSystemCapability::Write);
    assert!(!capabilities.is_empty());
    assert_eq!(1, capabilities.len());
    assert!(capabilities.contains(FileSystemCapability::Write));
}

#[test]
fn capability_set_iterates_and_formats_semantic_values() {
    let capabilities = FileSystemCapabilities::new()
        .with(FileSystemCapability::Read)
        .with(FileSystemCapability::Write);

    assert_eq!(
        vec![FileSystemCapability::Read, FileSystemCapability::Write],
        capabilities.iter().collect::<Vec<_>>(),
    );
    assert_eq!("{Read, Write}", format!("{capabilities:?}"));
}

#[test]
fn capability_all_matches_stable_iteration_order() {
    let all: &'static [FileSystemCapability] = FileSystemCapability::ALL;
    let capabilities = FileSystemCapability::ALL
        .iter()
        .copied()
        .fold(FileSystemCapabilities::new(), |capabilities, capability| {
            capabilities.with(capability)
        });
    assert_eq!(all, capabilities.iter().collect::<Vec<_>>(),);
}

#[test]
fn capability_set_reports_the_first_missing_dependency() {
    let capabilities =
        FileSystemCapabilities::new().with(FileSystemCapability::AtomicRename);

    assert_eq!(
        Some((
            FileSystemCapability::AtomicRename,
            FileSystemCapability::Rename,
        )),
        capabilities.missing_dependency()
    );
}

#[test]
fn capability_set_accepts_complete_dependency_sets() {
    let capabilities = FileSystemCapabilities::new()
        .with(FileSystemCapability::Read)
        .with(FileSystemCapability::RangeRead)
        .with(FileSystemCapability::Write)
        .with(FileSystemCapability::Append)
        .with(FileSystemCapability::Delete)
        .with(FileSystemCapability::RecursiveDelete)
        .with(FileSystemCapability::Rename)
        .with(FileSystemCapability::AtomicRename)
        .with(FileSystemCapability::Copy)
        .with(FileSystemCapability::ServerSideCopy);

    assert_eq!(None, capabilities.missing_dependency());
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
        FileSystemCapability::AtomicFileCopy,
        FileSystemCapability::AtomicTreeCopy,
        FileSystemCapability::DurableFileCopy,
        FileSystemCapability::DurableTreeCopy,
        FileSystemCapability::DurableRename,
    ];

    for (expected, capability) in capabilities.into_iter().enumerate() {
        assert_eq!(expected as u8, capability as u8);
    }
}
