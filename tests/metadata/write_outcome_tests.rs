// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    AchievedAtomicity,
    PublicationMethod,
    ResourceVersion,
    UserMetadata,
    WriteOutcome,
};

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
    let outcome =
        WriteOutcome::new(AchievedAtomicity::Atomic, PublicationMethod::Direct)
            .with_diagnostics(
                UserMetadata::new()
                    .with("request_id", "private-request-id")
                    .expect("ordinary diagnostic key must be accepted"),
            );
    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-request-id"));
}
