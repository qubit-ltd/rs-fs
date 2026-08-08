// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Cursor;
use std::sync::{
    Arc,
    Mutex,
};

use qubit_io::Input;

use qubit_fs::spi::{
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    FileSystemSpi,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedDirectoryStream,
    OpenedReader,
    OpenedTempDirectory,
    OpenedTempFile,
    OpenedWriter,
    RenameRequest,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    OpenedFileInfo,
    Path,
    PathConstraints,
    RenameFailureState,
    RenameOutcome,
    SymlinkPolicy,
};

struct ReaderSpi {
    wrong_opened_path: bool,
    read_requests: Arc<Mutex<Vec<usize>>>,
}

impl FileSystemSpi for ReaderSpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("reader-test").expect("valid id"),
                "reader-test",
                qubit_fs::PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with_guaranteed(qubit_fs::FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(qubit_fs::FileKind::File),
        ))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unsupported())
    }
    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader> {
        let path = if self.wrong_opened_path {
            Path::parse("/different").expect("valid path")
        } else {
            request.path().clone()
        };
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                FileSystemId::new("reader-test").expect("valid id"),
                path,
            ),
            Box::new(RecordingReader {
                inner: Cursor::new(b"bytes".to_vec()),
                read_requests: Arc::clone(&self.read_requests),
            }),
        ))
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(unsupported())
    }
    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        Err(unsupported())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unsupported())
    }
    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        Err(unsupported())
    }
    fn rename(
        &self,
        _: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unsupported(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(unsupported())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(unsupported())
    }
}

fn unsupported() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}

#[test]
fn test_open_reader_rejects_wrong_opened_identity() {
    let file_system = FileSystem::from_shared_spi(Arc::new(ReaderSpi {
        wrong_opened_path: true,
        read_requests: Arc::new(Mutex::new(Vec::new())),
    }))
    .expect("facade should open");
    let requested = Path::parse("/requested").expect("valid path");
    let error = file_system
        .open_reader(&requested, Default::default())
        .expect_err("wrong identity must be rejected");
    assert_eq!(error.kind(), FsErrorKind::ProviderContractViolation);
}

/// Delegates regular byte transfer while preserving the opened identity.
#[test]
fn test_open_reader_reads_bytes_and_exposes_identity() {
    let file_system = FileSystem::from_spi(ReaderSpi {
        wrong_opened_path: false,
        read_requests: Arc::new(Mutex::new(Vec::new())),
    })
    .expect("facade should open");
    let requested = Path::parse("/requested").expect("valid path");
    let mut reader = file_system
        .open_reader(&requested, Default::default())
        .expect("matching reader identity should open");
    assert_eq!(&requested, reader.info().path());
    assert!(!reader.is_buffered());
    assert!(format!("{reader:?}").contains("FileReader"));
    let mut bytes = [0; 5];
    assert_eq!(
        5,
        reader
            .read_fully(&mut bytes)
            .expect("reader should fill bytes")
    );
    assert_eq!(b"bytes", &bytes);
}

/// Reads a complete file through the facade convenience API and applies its
/// caller-supplied memory bound before extending the result buffer.
#[test]
fn test_read_all_returns_bytes_and_enforces_maximum() {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let file_system = FileSystem::from_spi(ReaderSpi {
        wrong_opened_path: false,
        read_requests: Arc::clone(&read_requests),
    })
    .expect("facade should open");
    let requested = Path::parse("/requested").expect("valid path");

    let bytes = file_system
        .read_all(&requested, Default::default(), 5)
        .expect("sufficient maximum should read every byte");
    assert_eq!(b"bytes", bytes.as_slice());
    read_requests.lock().expect("requests lock").clear();
    let error = file_system
        .read_all(&requested, Default::default(), 4)
        .expect_err("a too-small maximum must reject the complete chunk");
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(vec![5], *read_requests.lock().expect("requests lock"));
}

/// Reads at most the requested prefix while allowing a larger source file.
#[test]
fn test_read_prefix_returns_bounded_bytes() {
    let read_requests = Arc::new(Mutex::new(Vec::new()));
    let file_system = FileSystem::from_spi(ReaderSpi {
        wrong_opened_path: false,
        read_requests: Arc::clone(&read_requests),
    })
    .expect("facade should open");
    let requested = Path::parse("/requested").expect("valid path");

    assert_eq!(
        b"byt".as_slice(),
        file_system
            .read_prefix(&requested, Default::default(), 3)
            .expect("prefix should read")
            .as_slice()
    );
    read_requests.lock().expect("requests lock").clear();
    assert!(
        file_system
            .read_prefix(&requested, Default::default(), 0)
            .expect("zero prefix should open")
            .is_empty()
    );
    assert!(
        read_requests.lock().expect("requests lock").is_empty(),
        "zero prefix should not issue a read"
    );
}

/// Records the requested read slice while delegating byte transfer to a
/// cursor.
struct RecordingReader {
    inner: Cursor<Vec<u8>>,
    read_requests: Arc<Mutex<Vec<usize>>>,
}

impl Input for RecordingReader {
    type Item = u8;

    unsafe fn read_unchecked(
        &mut self,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> std::io::Result<usize> {
        self.read_requests
            .lock()
            .expect("requests lock")
            .push(count);
        std::io::Read::read(&mut self.inner, &mut output[index..index + count])
    }
}
