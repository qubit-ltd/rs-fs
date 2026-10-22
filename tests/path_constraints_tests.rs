// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::hint::black_box;

use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathForm;

#[test]
fn test_relative_constructor_is_executed_at_runtime() {
    let constructor: fn() -> PathConstraints = black_box(PathConstraints::relative);
    let either_constructor: fn() -> PathConstraints = black_box(PathConstraints::either);

    assert_eq!(PathForm::Relative, constructor().form());
    assert_eq!(PathForm::Either, either_constructor().form());
}
