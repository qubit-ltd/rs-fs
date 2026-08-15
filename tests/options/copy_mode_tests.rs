// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::copy::CopyMode;

#[test]
fn test_copy_mode_default_is_auto() {
    assert_eq!(CopyMode::Auto, CopyMode::default());
}
