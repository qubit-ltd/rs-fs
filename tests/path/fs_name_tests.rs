// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsName,
    FsPath,
};

#[test]
fn fs_name_prevents_child_path_escape() {
    let base = FsPath::parse_normalized("/safe").unwrap();
    let name = FsName::parse("child.txt").unwrap();

    assert_eq!("/safe/child.txt", base.child(&name).as_str());
    assert!(FsName::parse("").is_err());
    assert!(FsName::parse(".").is_err());
    assert!(FsName::parse("..").is_err());
    assert!(FsName::parse("/escape").is_err());
    assert!(FsName::parse("a/b").is_err());
    assert!(FsName::parse("bad\nname").is_err());
    assert_eq!("child.txt", name.to_string());
}
