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
    FileSystemLimit,
    FileSystemLimits,
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
        fs.read_all(&path, 4).expect("read_all should succeed"),
    );
}

#[test]
fn read_all_enforces_an_inclusive_caller_budget() {
    let fs = MockFs::default();
    let path = FsPath::parse("/file.txt").expect("path should parse");

    assert_eq!(b"data", fs.read_all(&path, 4).unwrap().as_slice());
    assert_eq!(b"data", fs.read_all(&path, usize::MAX).unwrap().as_slice());

    let error = fs.read_all(&path, 3).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn read_all_accepts_an_empty_resource_with_a_zero_budget() {
    let path = FsPath::parse("/empty").unwrap();
    let bytes = ExtFileSystem::new(ExtMode::InterruptedReader)
        .read_all(&path, 0)
        .unwrap();

    assert!(bytes.is_empty());
}

#[test]
fn read_all_preserves_an_error_raised_by_the_limit_probe() {
    let path = FsPath::parse("/probe-error").unwrap();
    let error = ExtFileSystem::new(ExtMode::ProbeErrorReader)
        .read_all(&path, 1)
        .unwrap_err();

    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn read_all_preserves_specific_standard_io_error_kinds() {
    let path = FsPath::parse("/timeout").expect("path should parse");

    let error = ExtFileSystem::new(ExtMode::TimedOutReader)
        .read_all(&path, 1)
        .expect_err("timed-out stream should fail");

    assert_eq!(FsErrorKind::Timeout, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn read_all_restores_embedded_file_system_error_context() {
    let path = FsPath::parse("/quota").expect("path should parse");

    let error = ExtFileSystem::new(ExtMode::EmbeddedFsErrorReader)
        .read_all(&path, 1)
        .expect_err("quota error should fail the read");

    assert_eq!(FsErrorKind::QuotaExceeded, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("stream-provider"), error.provider());
}

#[test]
fn extension_methods_preflight_provider_write_limits() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone()).with_limits(
        FileSystemLimits::unknown()
            .with_max_write_bytes(FileSystemLimit::Maximum(3)),
    );
    let path = FsPath::parse("/file").unwrap();

    let error = fs.write_all(&path, b"data").unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::Write, error.operation());
    assert_eq!(Some(&path), error.path());
    assert!(state.lock().unwrap().writes.is_empty());
}

#[test]
fn extension_methods_preflight_provider_path_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(3));
    let path = FsPath::parse("/file").unwrap();

    let read_error = MockFs::default()
        .with_limits(limits)
        .read_all(&path, 4)
        .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, read_error.kind());
    assert_eq!(FsOperation::Read, read_error.operation());

    let write_error = MockFs::default()
        .with_limits(limits)
        .write_all(&path, b"data")
        .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, write_error.kind());
    assert_eq!(FsOperation::Write, write_error.operation());
}

#[test]
fn test_read_all_and_write_all_error_branches() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone());
    let path = FsPath::parse("/file.txt").expect("path should parse");

    state.lock().expect("state lock should succeed").fail_read = true;
    assert!(fs.read_all(&path, 4).is_err());
    state.lock().expect("state lock should succeed").fail_read = false;

    state.lock().expect("state lock should succeed").fail_write = true;
    assert!(fs.write_all(&path, b"data").is_err());
    assert_eq!(1, state.lock().expect("state lock should succeed").aborts);
}

enum ExtMode {
    InterruptedReader,
    ProbeErrorReader,
    TimedOutReader,
    EmbeddedFsErrorReader,
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

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits =
            qubit_fs::FileSystemLimits::unknown();
        &LIMITS
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
            ExtMode::ProbeErrorReader => Ok(FileReader::new(
                ProbeErrorReader { emitted: false },
                opened_info(path),
            )),
            ExtMode::TimedOutReader => {
                Ok(FileReader::new(TimedOutReader, opened_info(path)))
            }
            ExtMode::EmbeddedFsErrorReader => {
                Ok(FileReader::new(EmbeddedFsErrorReader, opened_info(path)))
            }
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

struct ProbeErrorReader {
    emitted: bool,
}

struct TimedOutReader;

impl Read for TimedOutReader {
    fn read(&mut self, _buffer: &mut [u8]) -> IoResult<usize> {
        Err(IoError::from(IoErrorKind::TimedOut))
    }
}

struct EmbeddedFsErrorReader;

impl Read for EmbeddedFsErrorReader {
    fn read(&mut self, _buffer: &mut [u8]) -> IoResult<usize> {
        Err(FsError::new(
            FsErrorKind::QuotaExceeded,
            FsOperation::OpenReader,
            "provider quota exhausted",
        )
        .with_provider(
            ProviderId::new("stream-provider")
                .expect("provider id should parse"),
        )
        .into_io_error())
    }
}

impl Read for ProbeErrorReader {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        if self.emitted {
            Err(IoError::other("probe failed"))
        } else if buffer.is_empty() {
            Ok(0)
        } else {
            self.emitted = true;
            buffer[0] = b'x';
            Ok(1)
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
            .read_all(&path, 16)
            .is_err(),
    );
    assert_eq!(
        Vec::<u8>::new(),
        ExtFileSystem::new(ExtMode::InterruptedReader)
            .read_all(&path, 16)
            .unwrap(),
    );
    assert!(
        ExtFileSystem::new(ExtMode::OpenWriterError)
            .write_all(&path, b"data")
            .is_err(),
    );
}
