// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    CopyOptions,
    CreateDirOptions,
    DeleteOptions,
    FileKind,
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemExt,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    ListOptions,
    PathSemantics,
    ReadOptions,
    RenameOptions,
    TempDirOptions,
    TempFileOptions,
    WriteOptions,
};
use qubit_spi::ProviderId;

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_file_system_capabilities_exists_and_stat_work() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state);
    let path = FsPath::parse("/file.txt").expect("path should parse");

    assert!(
        fs.capabilities()
            .contains(FileSystemCapability::CreateDirectory)
    );
    assert!(!fs.exists(&path).expect("exists should succeed"));
    fs.write_all(&path, b"data")
        .expect("write_all should succeed");
    assert!(fs.exists(&path).expect("exists should succeed"));
    assert_eq!(
        4,
        fs.stat(&path)
            .expect("metadata should exist")
            .len
            .expect("file length should be set"),
    );
}

struct MinimalFileSystem {
    info: FileSystemInfo,
}

impl MinimalFileSystem {
    fn new() -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new("minimal").unwrap(),
                ProviderId::new("minimal").unwrap(),
                PathSemantics::Hierarchical,
            ),
        }
    }
}

impl FileSystemProperties for MinimalFileSystem {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }
}

impl FileSystem for MinimalFileSystem {
    fn stat(&self, path: &FsPath) -> qubit_fs::FsResult<FileMetadata> {
        match path.as_str() {
            "/present" => Ok(FileMetadata::new(FileKind::File)),
            "/missing" => Err(FsError::new(
                FsErrorKind::NotFound,
                FsOperation::Stat,
                "missing",
            )),
            _ => Err(FsError::new(
                FsErrorKind::PermissionDenied,
                FsOperation::Stat,
                "denied",
            )),
        }
    }
}

#[test]
fn file_system_defaults_are_safe_explicit_capability_failures() {
    let fs = MinimalFileSystem::new();
    let present = FsPath::parse("/present").unwrap();
    let missing = FsPath::parse("/missing").unwrap();
    let denied = FsPath::parse("/denied").unwrap();
    let target = FsPath::parse("/target").unwrap();

    assert!(fs.exists(&present).unwrap());
    assert!(!fs.exists(&missing).unwrap());
    let denied_error = fs.exists(&denied).unwrap_err();
    assert_eq!(FsErrorKind::PermissionDenied, denied_error.kind());
    assert_eq!(FsOperation::Exists, denied_error.operation());

    let failures = [
        fs.list(&present, ListOptions::default()).unwrap_err(),
        fs.open_reader(&present, ReadOptions::default())
            .unwrap_err(),
        fs.open_writer(&present, WriteOptions::default())
            .unwrap_err(),
        fs.create_dir(&present, CreateDirOptions::default())
            .unwrap_err(),
        fs.delete(&present, DeleteOptions::default()).unwrap_err(),
        fs.rename(&present, &target, RenameOptions::default())
            .unwrap_err(),
        fs.copy(&present, &target, CopyOptions::default())
            .unwrap_err(),
        fs.create_temp_file(TempFileOptions::default()).unwrap_err(),
        fs.create_temp_dir(TempDirOptions::default()).unwrap_err(),
    ];
    for error in failures {
        assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());
        assert!(error.required_capability().is_some());
    }
}
