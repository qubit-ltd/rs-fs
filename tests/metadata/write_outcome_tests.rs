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
    ResourceVersion,
    WriteOutcome,
};
use qubit_metadata::Metadata;

#[test]
fn write_outcome_reports_actual_publication_semantics() {
    let mut outcome = WriteOutcome::new(
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    );
    outcome.version = Some(ResourceVersion::new("v7"));

    assert_eq!(None, outcome.bytes_written);
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity);
    assert_eq!(PublicationMethod::AtomicRename, outcome.method);
    assert_eq!(
        Some("v7"),
        outcome.version.as_ref().map(ResourceVersion::as_str)
    );
}

#[test]
fn write_outcome_validates_and_safely_formats_diagnostics() {
    let error =
        WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct)
            .with_diagnostics(Metadata::new().with(
                "provider",
                serde_json::json!({"x-amz-signature": "plaintext"}),
            ))
            .expect_err("nested credential diagnostics must be rejected");
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());

    let outcome =
        WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct)
            .with_diagnostics(
                Metadata::new()
                    .with("request_id", "private-request-id".to_owned()),
            )
            .expect("safe diagnostic keys should be accepted");
    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-request-id"));
}
