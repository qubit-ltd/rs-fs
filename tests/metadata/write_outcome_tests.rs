// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::AchievedAtomicity;
use qubit_fs::PublicationMethod;
use qubit_fs::ResourceVersion;
use qubit_fs::UserMetadata;
use qubit_fs::WriteOutcome;

#[test]
fn write_outcome_reports_actual_publication_semantics() {
    let outcome = WriteOutcome::new(
        AchievedAtomicity::Atomic,
        PublicationMethod::AtomicRename,
    )
    .with_bytes_written(7)
    .with_version(ResourceVersion::new("v7"));

    assert_eq!(Some(7), outcome.bytes_written());
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert_eq!(PublicationMethod::AtomicRename, outcome.method());
    assert_eq!(Some("v7"), outcome.version().map(ResourceVersion::as_str));
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
    assert!(outcome.diagnostics().contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-request-id"));
}
