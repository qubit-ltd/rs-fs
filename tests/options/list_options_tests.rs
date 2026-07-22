// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::ListOptions;

#[test]
fn test_list_options_full_configuration_is_usable() {
    let options = ListOptions {
        recursive: true,
        follow_symlinks: true,
        include_metadata: true,
        page_size: Some(10),
        prefix: Some("a".to_owned()),
    };

    assert!(options.recursive);
    assert!(options.follow_symlinks);
    assert!(options.include_metadata);
    assert_eq!(Some(10), options.page_size);
    assert_eq!(Some("a"), options.prefix.as_deref());
}
