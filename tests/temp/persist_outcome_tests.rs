// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    AchievedAtomicity,
    Path,
    PersistOutcome,
    PublicationMethod,
    UserMetadata,
};

/// Verifies a successful temporary publication exposes its target, achieved
/// guarantees, and non-sensitive provider diagnostics.
#[test]
fn test_persist_outcome_preserves_publication_details_and_diagnostics() {
    let target =
        Path::parse("published/report.txt").expect("path should parse");
    let outcome = PersistOutcome::new(
        target.clone(),
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_diagnostics(
        UserMetadata::new()
            .with("storage_class", "regional")
            .expect("diagnostic key should be safe"),
    );

    assert_eq!(target, outcome.target);
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity);
    assert_eq!(PublicationMethod::AtomicRename, outcome.method);
    assert!(outcome.diagnostics.contains_key("storage_class"));
}
