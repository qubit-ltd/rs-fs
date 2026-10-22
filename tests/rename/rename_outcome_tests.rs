// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::UserMetadata;
use qubit_fs::rename::RenameOutcome;

#[test]
fn test_outcome_accessors_are_executed_at_runtime() {
    let source = Path::parse("/source").expect("valid source path");
    let target = Path::parse("/target").expect("valid target path");
    let outcome = RenameOutcome::new(
        source.clone(),
        target.clone(),
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_durable(true)
    .with_diagnostics(UserMetadata::new());

    assert_eq!(outcome.source(), &source);
    assert_eq!(outcome.target(), &target);
    assert_eq!(outcome.atomicity(), AchievedAtomicity::Atomic);
    assert_eq!(outcome.method(), PublicationMethod::AtomicRename);
    assert!(outcome.durable());
    assert!(outcome.diagnostics().is_empty());
}
