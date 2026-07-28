// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsAuthority,
    FsUriAuthority,
};

#[test]
fn fs_uri_authority_distinguishes_absent_empty_and_present_components() {
    let authority = FsAuthority::new("bucket").expect("authority should parse");

    assert_ne!(FsUriAuthority::Absent, FsUriAuthority::Empty);
    assert_eq!(
        FsUriAuthority::Present(authority.clone()),
        FsUriAuthority::Present(authority),
    );
}
