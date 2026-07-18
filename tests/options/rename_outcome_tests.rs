// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    AchievedAtomicity,
    FsErrorKind,
    PublicationMethod,
    RenameOutcome,
};
use qubit_metadata::Metadata;

#[test]
fn rename_outcome_reports_actual_method_and_atomicity() {
    let outcome = RenameOutcome::new(
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    );

    assert_eq!(AchievedAtomicity::NonAtomic, outcome.atomicity);
    assert_eq!(PublicationMethod::CopyThenDelete, outcome.method);
}

#[test]
fn rename_outcome_rejects_nested_sensitive_diagnostics() {
    let error = RenameOutcome::new(
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_diagnostics(Metadata::new().with(
        "provider",
        serde_json::json!({"items": [{"secret_key": "plaintext"}]}),
    ))
    .expect_err("nested credential diagnostics must be rejected");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());

    let outcome = RenameOutcome::new(
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    )
    .with_diagnostics(
        Metadata::new().with("request_id", "private-rename-id".to_owned()),
    )
    .expect("safe diagnostics should be accepted");
    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-rename-id"));
}
