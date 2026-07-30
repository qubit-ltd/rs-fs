// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ServerSidePreference;

#[test]
fn test_server_side_preference_default_is_disable() {
    assert_eq!(
        ServerSidePreference::Disable,
        ServerSidePreference::default()
    );
}
