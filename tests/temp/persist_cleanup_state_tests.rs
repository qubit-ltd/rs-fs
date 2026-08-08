// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::PersistCleanupState;

#[test]
fn persist_cleanup_states_are_debuggable_and_distinct() {
    assert_ne!(
        PersistCleanupState::Complete,
        PersistCleanupState::ResidualTemporaryContainer,
    );
    assert_eq!("Complete", format!("{:?}", PersistCleanupState::Complete),);
}
