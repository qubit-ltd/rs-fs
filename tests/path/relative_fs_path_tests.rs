// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FsPath,
    RelativeFsPath,
};

#[test]
fn relative_path_prevents_descendant_escape() {
    let base = FsPath::parse_normalized("/safe").unwrap();
    let relative = RelativeFsPath::parse("a/./b").unwrap();

    assert_eq!("a/b", relative.as_str());
    assert_eq!("/safe/a/b", base.join_relative(&relative).as_str());
    assert!(RelativeFsPath::parse("/escape").is_err());
    assert!(RelativeFsPath::parse("../escape").is_err());
    assert!(RelativeFsPath::parse("a/../../escape").is_err());
    assert!(RelativeFsPath::parse("").is_err());
    assert!(RelativeFsPath::parse(".").is_err());
    assert!(RelativeFsPath::parse("a/..").is_err());
    assert!(RelativeFsPath::parse("bad\npath").is_err());
    assert_eq!("a/b", relative.to_string());
}
