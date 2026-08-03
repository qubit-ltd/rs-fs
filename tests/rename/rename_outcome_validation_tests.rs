// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public rename outcome coverage for the shared facade validation path.

use qubit_fs::{
    AchievedAtomicity,
    Path,
    PublicationMethod,
    RenameOutcome,
};

/// Verifies the outcome facts consumed by the shared sync/async validator.
#[test]
fn test_rename_outcome_preserves_durable_publication_fact() {
    let source = Path::parse("/source").expect("source path should parse");
    let target = Path::parse("/target").expect("target path should parse");
    let outcome = RenameOutcome::new(
        source,
        target,
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_durable(true);
    assert!(outcome.durable());
}
