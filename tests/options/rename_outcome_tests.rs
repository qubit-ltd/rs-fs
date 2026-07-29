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
    PublicationMethod,
    RenameOutcome,
    UserMetadata,
};

#[test]
fn rename_outcome_reports_actual_method_and_atomicity() {
    let outcome = RenameOutcome::new(
        Path::parse("/source").expect("path must parse"),
        Path::parse("/target").expect("path must parse"),
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    );

    assert_eq!(AchievedAtomicity::NonAtomic, outcome.atomicity());
    assert_eq!(PublicationMethod::CopyThenDelete, outcome.method());
}

#[test]
fn rename_outcome_preserves_validated_diagnostics() {
    let outcome = RenameOutcome::new(
        Path::parse("/source").expect("path must parse"),
        Path::parse("/target").expect("path must parse"),
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    )
    .with_diagnostics(
        UserMetadata::new()
            .with("request_id", "private-rename-id")
            .unwrap(),
    );
    assert!(outcome.diagnostics().contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-rename-id"));
}
