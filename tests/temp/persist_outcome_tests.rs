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
    FsPath,
    PersistOutcome,
    PublicationMethod,
};
use qubit_metadata::Metadata;

#[test]
fn persist_outcome_rejects_nested_sensitive_diagnostics() {
    let error = PersistOutcome::new(
        FsPath::parse("/final").unwrap(),
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_diagnostics(Metadata::new().with(
        "provider",
        serde_json::json!({"items": [{"access_token": "plaintext"}]}),
    ))
    .expect_err("nested credential diagnostics must be rejected");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());

    let outcome = PersistOutcome::new(
        FsPath::parse("/final").unwrap(),
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    )
    .with_diagnostics(
        Metadata::new().with("request_id", "private-persist-id".to_owned()),
    )
    .expect("safe diagnostics should be accepted");
    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-persist-id"));
}
