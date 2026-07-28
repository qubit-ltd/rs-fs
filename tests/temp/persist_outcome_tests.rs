// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    AchievedAtomicity,
    FsPath,
    PersistOutcome,
    PublicationMethod,
    UserMetadata,
};

#[test]
fn persist_outcome_preserves_safe_diagnostics() {
    let outcome = PersistOutcome::new(
        FsPath::parse("/final").unwrap(),
        AchievedAtomicity::NonAtomic,
        PublicationMethod::CopyThenDelete,
    )
    .with_diagnostics(
        UserMetadata::new()
            .with("request_id", "private-persist-id")
            .unwrap(),
    );

    assert!(outcome.diagnostics.contains_key("request_id"));
    assert!(!format!("{outcome:?}").contains("private-persist-id"));
}
