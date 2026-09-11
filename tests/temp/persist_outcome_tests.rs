// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::UserMetadata;
use qubit_fs::temp::PersistCleanupState;
use qubit_fs::temp::PersistOutcome;

/// Verifies a successful temporary publication exposes its target, achieved
/// guarantees, and non-sensitive provider diagnostics.
#[test]
fn test_persist_outcome_preserves_publication_details_and_diagnostics() {
    let target = Path::parse("published/report.txt").expect("path should parse");
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

    assert_eq!(&target, outcome.target());
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert_eq!(PublicationMethod::AtomicRename, outcome.method());
    assert!(outcome.diagnostics().contains_key("storage_class"));
    assert_eq!(PersistCleanupState::Complete, outcome.cleanup_state());

    let residual = outcome.with_cleanup_state(PersistCleanupState::ResidualTemporaryContainer);
    assert_eq!(
        PersistCleanupState::ResidualTemporaryContainer,
        residual.cleanup_state()
    );
}

#[test]
fn persist_outcome_accessors_are_callable_directly() {
    let target = Path::parse("published/report.txt").expect("path should parse");
    let outcome = PersistOutcome::new(target.clone(), AchievedAtomicity::Atomic, PublicationMethod::Direct);
    let target_accessor: fn(&PersistOutcome) -> &Path = PersistOutcome::target;
    let atomicity: fn(&PersistOutcome) -> AchievedAtomicity = PersistOutcome::atomicity;
    let method: fn(&PersistOutcome) -> PublicationMethod = PersistOutcome::method;
    let diagnostics: fn(&PersistOutcome) -> &_ = PersistOutcome::diagnostics;
    let cleanup_state: fn(&PersistOutcome) -> PersistCleanupState = PersistOutcome::cleanup_state;

    assert_eq!(&target, target_accessor(&outcome));
    assert_eq!(AchievedAtomicity::Atomic, atomicity(&outcome));
    assert_eq!(PublicationMethod::Direct, method(&outcome));
    assert!(diagnostics(&outcome).is_empty());
    assert_eq!(PersistCleanupState::Complete, cleanup_state(&outcome));
}
