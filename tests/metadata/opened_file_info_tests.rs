// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
use qubit_fs::FileSystemId;
use qubit_fs::OpenedFileInfo;
use qubit_fs::Path;

/// Verifies an opened-file snapshot preserves identity and can carry metadata
/// already observed by the provider during open.
#[test]
fn test_opened_file_info_preserves_identity_and_optional_metadata() {
    let filesystem_id =
        FileSystemId::new("opened-info").expect("filesystem id should parse");
    let path = Path::parse("reports/today.txt").expect("path should parse");
    let info = OpenedFileInfo::new(filesystem_id.clone(), path.clone());

    assert_eq!(&filesystem_id, info.filesystem_id());
    assert_eq!(&path, info.path());
    assert_eq!(None, info.metadata());

    let metadata = FileMetadata::new(FileKind::File);
    let info = info.with_metadata(metadata.clone());
    assert_eq!(Some(&metadata), info.metadata());
}
