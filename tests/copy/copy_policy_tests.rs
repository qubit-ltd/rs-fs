// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Public option combinations covered by the stream-copy policy tests.

use qubit_fs::copy::CopyMode;
use qubit_fs::copy::CopyOptions;
use qubit_fs::metadata::SymlinkPolicy;

#[test]
fn tree_mode_is_explicit_and_symlink_override_is_preserved() {
    let options = CopyOptions::tree().with_symlink_policy(SymlinkPolicy::FollowWithinFileSystem);

    assert_eq!(CopyMode::Tree, options.mode());
    assert_eq!(
        Some(SymlinkPolicy::FollowWithinFileSystem),
        options.symlink_policy_override(),
    );
}
