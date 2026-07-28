// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::{
    FileLocation,
    FileSystemId,
    FsPath,
    FsUri,
};

#[test]
fn file_location_preserves_opened_identity_path_and_optional_uri() {
    let file_system_id = FileSystemId::new("mock-instance").unwrap();
    let path = FsPath::parse("/data/file.txt").unwrap();
    let location = FileLocation::new(file_system_id.clone(), path.clone());

    assert_eq!(&file_system_id, location.file_system_id());
    assert_eq!(&path, location.path());
    assert_eq!(None, location.uri());

    let uri = FsUri::parse("mock://host/data/file.txt").unwrap();
    let location = location.with_uri(uri.clone());
    assert_eq!(Some(&uri), location.uri());
}
