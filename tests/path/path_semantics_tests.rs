// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::path::PathSemantics;

#[test]
fn test_path_semantics_hierarchical_variant_is_comparable() {
    let semantics = PathSemantics::Hierarchical;

    assert!(matches!(semantics, PathSemantics::Hierarchical));
}

#[test]
fn path_semantics_defaults_to_hierarchical() {
    assert_eq!(PathSemantics::Hierarchical, PathSemantics::default());
}
