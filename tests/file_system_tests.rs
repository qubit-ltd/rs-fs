// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the synchronous filesystem facade.

use std::sync::{
    Arc,
    atomic::{
        AtomicUsize,
        Ordering,
    },
};

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
    Path,
    PathConstraints,
    RenameOutcome,
};

struct CountingSpi {
    properties: FileSystemProperties,
    property_calls: Arc<AtomicUsize>,
    stat_calls: Arc<AtomicUsize>,
    wrong_stat_path: bool,
}

impl CountingSpi {
    fn unsupported() -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Other,
            "unused test operation",
        )
    }
}

impl FileSystemSpi for CountingSpi {
    fn properties(&self) -> FileSystemProperties {
        self.property_calls.fetch_add(1, Ordering::SeqCst);
        self.properties.clone()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.stat_calls.fetch_add(1, Ordering::SeqCst);
        let path = if self.wrong_stat_path {
            Path::parse("/wrong").expect("test path should parse")
        } else {
            request.path().clone()
        };
        Ok(StatResponse::new(
            path,
            FileMetadata::new(qubit_fs::FileKind::File),
        ))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(Self::unsupported())
    }
    fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Err(Self::unsupported())
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(Self::unsupported())
    }
    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        Err(Self::unsupported())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(Self::unsupported())
    }
    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        Err(Self::unsupported())
    }
    fn rename(
        &self,
        _: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            Self::unsupported(),
            qubit_fs::RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(Self::unsupported())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(Self::unsupported())
    }
}

#[test]
fn test_file_system_from_spi_caches_properties_snapshot() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            qubit_fs::PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    );
    let properties = properties.expect("properties should be valid");
    let property_calls = Arc::new(AtomicUsize::new(0));
    let stat_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::clone(&property_calls),
        stat_calls: Arc::clone(&stat_calls),
        wrong_stat_path: false,
    })
    .expect("facade should construct");
    let clone = filesystem.clone();
    assert_eq!(
        filesystem.properties().info().provider_id(),
        clone.properties().info().provider_id()
    );
    assert_eq!(1, property_calls.load(Ordering::SeqCst));
    let relative = Path::parse("relative").expect("relative path should parse");
    let error = filesystem
        .stat(&relative)
        .expect_err("absolute-only filesystem should reject relative path");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(0, stat_calls.load(Ordering::SeqCst));
}

#[test]
fn test_stat_rejects_path_with_different_semantics_before_spi_call() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("object-store").expect("test id should be valid"),
            "test",
            qubit_fs::PathSemantics::ObjectKey,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::either(),
    )
    .expect("properties should be valid");
    let stat_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::clone(&stat_calls),
        wrong_stat_path: false,
    })
    .expect("facade should construct");
    let hierarchical =
        Path::parse("object").expect("hierarchical path should parse");
    let error = filesystem
        .stat(&hierarchical)
        .expect_err("different path semantics must fail before SPI");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(0, stat_calls.load(Ordering::SeqCst));
}

#[test]
fn test_stat_rejects_provider_response_for_a_different_path() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            qubit_fs::PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
    )
    .expect("properties should be valid");
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: true,
    })
    .expect("facade should construct");
    let path = Path::parse("/requested").expect("path should parse");
    let error = filesystem
        .stat(&path)
        .expect_err("mismatched provider response must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}
