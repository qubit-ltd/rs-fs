// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public deadline option contract tests.

use std::time::Duration;

use qubit_fs::copy::CopyOptions;

/// Ensures a configured deadline survives option construction unchanged.
#[test]
fn test_copy_options_preserves_cumulative_deadline() {
    let deadline = Duration::from_secs(1);
    let options = CopyOptions::default().with_deadline(Some(deadline));

    assert_eq!(Some(deadline), options.deadline());
}

/// Ensures an absent deadline keeps copy operations unbounded by elapsed time.
#[test]
fn test_copy_options_default_has_no_deadline() {
    assert_eq!(None, CopyOptions::default().deadline());
}
