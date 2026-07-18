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

use std::io::{
    Error as IoError,
    ErrorKind as IoErrorKind,
    Read,
    Result as IoResult,
};

use qubit_fs::{
    FileKind,
    FileLocation,
    FileMetadata,
    FileReader,
    FileSystem,
    FileSystemCapabilities,
    FileSystemExt,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    OpenedFileInfo,
    PathSemantics,
    ReadOptions,
    WriteOptions,
};
use qubit_spi::ProviderId;

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_read_all_and_write_all_success_paths() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state);
    let path = FsPath::parse("/file.txt").expect("path should parse");

    fs.write_all(&path, b"data")
        .expect("write_all should succeed");
    assert_eq!(
        b"data".to_vec(),
        fs.read_all(&path).expect("read_all should succeed"),
    );
}

#[test]
fn test_read_all_and_write_all_error_branches() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone());
    let path = FsPath::parse("/file.txt").expect("path should parse");

    state.lock().expect("state lock should succeed").fail_read = true;
    assert!(fs.read_all(&path).is_err());
    state.lock().expect("state lock should succeed").fail_read = false;

    state.lock().expect("state lock should succeed").fail_write = true;
    assert!(fs.write_all(&path, b"data").is_err());
    assert_eq!(1, state.lock().expect("state lock should succeed").aborts);
}

enum ExtMode {
    InterruptedReader,
    OpenReaderError,
    OpenWriterError,
}

struct ExtFileSystem {
    info: FileSystemInfo,
    mode: ExtMode,
}

impl ExtFileSystem {
    fn new(mode: ExtMode) -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new("ext").unwrap(),
                ProviderId::new("ext").unwrap(),
                PathSemantics::Hierarchical,
            ),
            mode,
        }
    }
}

impl FileSystemProperties for ExtFileSystem {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }
}

impl FileSystem for ExtFileSystem {
    fn stat(&self, _path: &FsPath) -> qubit_fs::FsResult<FileMetadata> {
        Ok(FileMetadata::new(FileKind::File))
    }

    fn open_reader(
        &self,
        path: &FsPath,
        _options: ReadOptions,
    ) -> qubit_fs::FsResult<FileReader> {
        match self.mode {
            ExtMode::InterruptedReader => Ok(FileReader::new(
                InterruptedOnceReader(true),
                opened_info(path),
            )),
            _ => Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::OpenReader,
                "open reader failed",
            )),
        }
    }

    fn open_writer(
        &self,
        _path: &FsPath,
        _options: WriteOptions,
    ) -> qubit_fs::FsResult<FileWriter> {
        Err(FsError::new(
            FsErrorKind::Io,
            FsOperation::OpenWriter,
            "open writer failed",
        ))
    }
}

struct InterruptedOnceReader(bool);

impl Read for InterruptedOnceReader {
    fn read(&mut self, _buffer: &mut [u8]) -> IoResult<usize> {
        if self.0 {
            self.0 = false;
            Err(IoError::new(IoErrorKind::Interrupted, "retry"))
        } else {
            Ok(0)
        }
    }
}

fn opened_info(path: &FsPath) -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("ext").unwrap(),
        path.clone(),
    ))
}

#[test]
fn extension_methods_handle_open_failures_and_interrupted_reads() {
    let path = FsPath::parse("/file").unwrap();

    assert!(
        ExtFileSystem::new(ExtMode::OpenReaderError)
            .read_all(&path)
            .is_err(),
    );
    assert_eq!(
        Vec::<u8>::new(),
        ExtFileSystem::new(ExtMode::InterruptedReader)
            .read_all(&path)
            .unwrap(),
    );
    assert!(
        ExtFileSystem::new(ExtMode::OpenWriterError)
            .write_all(&path, b"data")
            .is_err(),
    );
}
