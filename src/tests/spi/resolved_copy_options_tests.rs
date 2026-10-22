// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for facade-resolved copy options.

use crate::copy::CopyOptions;
use crate::metadata::SymlinkPolicy;
use crate::spi::ResolvedCopyOptions;

#[test]
fn test_resolved_copy_options_expose_their_values() {
    let options = CopyOptions::default();
    let resolved = ResolvedCopyOptions::new(options.clone(), SymlinkPolicy::FollowWithinFileSystem);

    assert_eq!(&options, resolved.options());
    assert_eq!(SymlinkPolicy::FollowWithinFileSystem, resolved.symlink_policy());
}
