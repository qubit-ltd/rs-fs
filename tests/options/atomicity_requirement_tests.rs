// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::AtomicityRequirement;

#[test]
fn atomicity_requirement_defaults_to_preferred() {
    assert_eq!(
        AtomicityRequirement::Preferred,
        AtomicityRequirement::default(),
    );
}

#[test]
fn atomicity_requirement_distinguishes_all_three_contracts() {
    assert_ne!(
        AtomicityRequirement::Required,
        AtomicityRequirement::Preferred,
    );
    assert_ne!(
        AtomicityRequirement::Preferred,
        AtomicityRequirement::NotRequired,
    );
}
