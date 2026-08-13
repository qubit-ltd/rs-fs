// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Cursor;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOutcome;
use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
use qubit_fs::FileSystem;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::FileSystemLimits;
use qubit_fs::FileSystemProperties;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::FsResult;
use qubit_fs::OpenedFileInfo;
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_io::Input;

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
                PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
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
            ).with_metadata(FileMetadata::new(FileKind::File).with_len(Some(5))),
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
    assert!(read_requests.lock().expect("requests lock").is_empty());
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
