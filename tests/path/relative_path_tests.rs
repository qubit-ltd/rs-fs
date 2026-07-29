// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for safe relative paths.

use qubit_fs::RelativePath;

/// Verifies relative paths cannot escape their logical base.
#[test]
fn test_relative_path_rejects_escape() {
    assert!(RelativePath::parse("../secret").is_err());
}
