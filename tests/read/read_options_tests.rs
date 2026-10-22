// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::hint::black_box;

use qubit_fs::metadata::ResourceVersion;
use qubit_fs::read::ReadOptions;

#[test]
fn test_version_accessor_is_executed_at_runtime() {
    let if_match: for<'a> fn(&'a ReadOptions) -> Option<&'a ResourceVersion> = black_box(ReadOptions::if_match);
    let options = ReadOptions::default().with_if_match(Some(ResourceVersion::new("v1")));

    assert_eq!(Some("v1"), if_match(&options).map(ResourceVersion::as_str));
}
