// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{DirEntry, FileKind, FsPath};

#[test]
fn test_dir_entry_new_derives_file_name() {
    let entry = DirEntry::new(
        FsPath::parse("/dir/file.txt").expect("path should parse"),
        FileKind::File,
    );

    assert_eq!("file.txt", entry.name);
    assert_eq!(FileKind::File, entry.kind);
}
