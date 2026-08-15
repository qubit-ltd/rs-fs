// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemCapabilitySupport;

#[test]
fn test_support_statuses_are_distinct_and_overwrite_previous_state() {
    let capabilities = FileSystemCapabilities::new()
        .with_conditional(FileSystemCapability::AtomicRename)
        .with_guaranteed(FileSystemCapability::AtomicRename);

    assert_eq!(
        FileSystemCapabilitySupport::Guaranteed,
        capabilities.support(FileSystemCapability::AtomicRename),
    );
    assert!(capabilities.supports(FileSystemCapability::AtomicRename));
    assert!(capabilities.guarantees(FileSystemCapability::AtomicRename));

    let capabilities = capabilities.set_support(
        FileSystemCapability::AtomicRename,
        FileSystemCapabilitySupport::Conditional,
    );
    assert_eq!(
        FileSystemCapabilitySupport::Conditional,
        capabilities.support(FileSystemCapability::AtomicRename),
    );
    assert!(capabilities.supports(FileSystemCapability::AtomicRename));
    assert!(!capabilities.guarantees(FileSystemCapability::AtomicRename));
}

#[test]
fn capability_set_reports_typed_guarantees() {
    let capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Read)
        .with_guaranteed(FileSystemCapability::AtomicRename);

    assert!(capabilities.supports(FileSystemCapability::Read));
    assert!(capabilities.supports(FileSystemCapability::AtomicRename));
    assert!(!capabilities.supports(FileSystemCapability::AtomicReplace));
}

#[test]
fn capability_set_supports_mutation_and_an_empty_default() {
    let mut capabilities = FileSystemCapabilities::default();
    assert!(capabilities.is_empty());
    assert_eq!(0, capabilities.len());
    assert!(!capabilities.supports(FileSystemCapability::Write));

    capabilities = capabilities.with_guaranteed(FileSystemCapability::Write);
    assert!(!capabilities.is_empty());
    assert_eq!(1, capabilities.len());
    assert!(capabilities.supports(FileSystemCapability::Write));
}

#[test]
fn capability_set_iterates_and_formats_semantic_values() {
    let capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Read)
        .with_guaranteed(FileSystemCapability::Write);

    assert_eq!(
        vec![FileSystemCapability::Read, FileSystemCapability::Write],
        capabilities.iter().collect::<Vec<_>>(),
    );
    assert_eq!(
        "{Read: Guaranteed, Write: Guaranteed}",
        format!("{capabilities:?}"),
    );
}

#[test]
fn capability_all_matches_stable_iteration_order() {
    let all: &'static [FileSystemCapability] = FileSystemCapability::ALL;
    let capabilities = FileSystemCapability::ALL
        .iter()
        .copied()
        .fold(FileSystemCapabilities::new(), |capabilities, capability| {
            capabilities.with_guaranteed(capability)
        });
    assert_eq!(all, capabilities.iter().collect::<Vec<_>>(),);
}

#[test]
fn capability_set_reports_the_first_missing_dependency() {
    let capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::AtomicRename);

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
        .with_guaranteed(FileSystemCapability::Read)
        .with_guaranteed(FileSystemCapability::RangeRead)
        .with_guaranteed(FileSystemCapability::Write)
        .with_guaranteed(FileSystemCapability::Append)
        .with_guaranteed(FileSystemCapability::Delete)
        .with_guaranteed(FileSystemCapability::RecursiveDelete)
        .with_guaranteed(FileSystemCapability::Rename)
        .with_guaranteed(FileSystemCapability::AtomicRename)
        .with_guaranteed(FileSystemCapability::Copy)
        .with_guaranteed(FileSystemCapability::ServerSideCopy);

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
