// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::hint::black_box;

use qubit_fs::copy::CopyMethod;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::copy::CopyStats;
use qubit_fs::copy::MetadataPreservePolicy;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::NonSensitiveMetadata;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::metadata::UserMetadata;

#[test]
fn test_copy_outcome_accessors_are_executed_at_runtime() {
    let constructor: fn(CopyStats, CopyMethod, AchievedAtomicity) -> CopyOutcome = black_box(CopyOutcome::new);
    let metadata: fn(&CopyOutcome) -> MetadataPreservePolicy = black_box(CopyOutcome::metadata);
    let target_version: for<'a> fn(&'a CopyOutcome) -> Option<&'a ResourceVersion> =
        black_box(CopyOutcome::target_version);
    let diagnostics: fn(&CopyOutcome) -> &NonSensitiveMetadata = black_box(CopyOutcome::diagnostics);
    let with_target_version: fn(CopyOutcome, ResourceVersion) -> CopyOutcome =
        black_box(CopyOutcome::with_target_version);
    let with_diagnostics: fn(CopyOutcome, UserMetadata) -> CopyOutcome = black_box(CopyOutcome::with_diagnostics);

    let outcome = with_diagnostics(
        with_target_version(
            constructor(CopyStats::default(), CopyMethod::Native, AchievedAtomicity::Atomic),
            ResourceVersion::new("generation-7"),
        ),
        UserMetadata::new(),
    );
    assert_eq!(MetadataPreservePolicy::None, metadata(&outcome));
    assert_eq!(
        Some("generation-7"),
        target_version(&outcome).map(ResourceVersion::as_str)
    );
    assert!(diagnostics(&outcome).is_empty());
}

#[test]
fn test_copy_outcome_new_stores_stats_and_method() {
    let stats = CopyStats {
        files: 1,
        bytes: 4,
        ..Default::default()
    };
    let outcome = CopyOutcome::new(stats, CopyMethod::Mixed, AchievedAtomicity::NonAtomic);

    assert_eq!(1, outcome.stats().files);
    assert_eq!(4, outcome.stats().bytes);
    assert_eq!(CopyMethod::Mixed, outcome.method());
    assert_eq!(AchievedAtomicity::NonAtomic, outcome.atomicity());
    assert!(!outcome.durable());
    assert!(outcome.diagnostics().is_empty());
}

#[test]
fn test_copy_outcome_preserves_validated_diagnostics() {
    let outcome = CopyOutcome::new(CopyStats::default(), CopyMethod::Streamed, AchievedAtomicity::NonAtomic)
        .with_diagnostics(
            UserMetadata::new()
                .with("request_id", "private-copy-id")
                .expect("ordinary copy diagnostic key must be accepted"),
        );
    assert!(outcome.diagnostics().contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-copy-id"));
}

/// Verifies providers can explicitly report completed durability
/// synchronization.
#[test]
fn test_copy_outcome_with_durable_reports_true() {
    let outcome =
        CopyOutcome::new(CopyStats::default(), CopyMethod::Native, AchievedAtomicity::Atomic).with_durable(true);
    assert!(outcome.durable());
}

/// Verifies providers can report the achieved metadata policy and destination
/// version needed by facade contract checks.
#[test]
fn test_copy_outcome_reports_metadata_and_target_version() {
    let outcome = CopyOutcome::new(CopyStats::default(), CopyMethod::Native, AchievedAtomicity::Atomic)
        .with_metadata(MetadataPreservePolicy::Portable)
        .with_target_version(ResourceVersion::new("generation-7"));
    assert_eq!(MetadataPreservePolicy::Portable, outcome.metadata());
    assert_eq!(
        Some("generation-7"),
        outcome.target_version().map(ResourceVersion::as_str)
    );
}
