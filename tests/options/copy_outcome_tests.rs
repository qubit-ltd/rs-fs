// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    AchievedAtomicity,
    CopyMethod,
    CopyOutcome,
    CopyStats,
    UserMetadata,
};

#[test]
fn test_copy_outcome_new_stores_stats_and_method() {
    let stats = CopyStats {
        files: 1,
        bytes: 4,
        ..Default::default()
    };
    let outcome = CopyOutcome::new(
        stats,
        CopyMethod::Mixed,
        AchievedAtomicity::NonAtomic,
    );

    assert_eq!(1, outcome.stats.files);
    assert_eq!(4, outcome.stats.bytes);
    assert_eq!(CopyMethod::Mixed, outcome.method);
    assert_eq!(AchievedAtomicity::NonAtomic, outcome.atomicity);
}

#[test]
fn copy_outcome_rejects_nested_sensitive_diagnostics() {
    let outcome = CopyOutcome::new(
        CopyStats::default(),
        CopyMethod::Stream,
        AchievedAtomicity::NonAtomic,
    )
    .with_diagnostics(
        UserMetadata::new()
            .with("request_id", "private-copy-id")
            .unwrap(),
    )
    .expect("safe diagnostics should be accepted");
    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-copy-id"));
}
