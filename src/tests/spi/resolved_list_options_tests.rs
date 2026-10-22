// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for facade-resolved listing options.

use crate::directory::ListOptions;
use crate::metadata::SymlinkPolicy;
use crate::spi::ResolvedListOptions;

#[test]
fn test_resolved_list_options_expose_their_values() {
    let options = ListOptions::default().with_recursive(true);
    let resolved = ResolvedListOptions::new(options.clone(), SymlinkPolicy::FollowWithinFileSystem);

    assert_eq!(&options, resolved.options());
    assert_eq!(SymlinkPolicy::FollowWithinFileSystem, resolved.symlink_policy());
}
