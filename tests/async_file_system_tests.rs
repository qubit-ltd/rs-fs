// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External contract coverage for the asynchronous facade boundary.

#![cfg(feature = "async")]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::CopyOptions;
use qubit_fs::CreateDirectoryOptions;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOptions;
use qubit_fs::DeleteOutcome;
use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
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
use qubit_fs::ListOptions;
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::ReadOptions;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOptions;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::TempDirectoryOptions;
use qubit_fs::TempFileOptions;
use qubit_fs::WriteOptions;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedAsyncDirectoryStream;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::OpenedAsyncTempDirectory;
use qubit_fs::spi::OpenedAsyncTempFile;
use qubit_fs::spi::OpenedAsyncWriter;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

struct PropertiesOnlySpi;

impl AsyncFileSystemSpi for PropertiesOnlySpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-test")
                    .expect("test id should be valid"),
                "async-test",
                PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Copy),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }

    fn stat<'a>(
        &'a self,
        _: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { Err(unused()) })
    }
    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        Box::pin(async { Err(unused()) })
    }
    fn open_reader<'a>(
        &'a self,
        _: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }
    fn open_writer<'a>(
        &'a self,
        _: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        Box::pin(async { Err(unused()) })
    }
    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_file<'a>(
        &'a self,
        _: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_directory<'a>(
        &'a self,
        _: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn rename<'a>(
        &'a self,
        _: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        Box::pin(async {
            Err(SpiRenameFailure::new(
                unused(),
                RenameFailureState::Unchanged,
            ))
        })
    }
    fn create_temp_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }
    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }
}

/// Provider that relies on every optional asynchronous SPI default method.
struct DefaultAsyncSpi {
    properties: FileSystemProperties,
}

impl AsyncFileSystemSpi for DefaultAsyncSpi {
    /// Returns the properties used to enable each default operation path.
    fn properties(&self) -> FileSystemProperties {
        self.properties.clone()
    }

    /// Supplies the required metadata implementation for the test provider.
    fn stat<'a>(
        &'a self,
        request: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            Ok(StatResponse::new(
                request.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        })
    }
}

/// Builds valid properties that advertise all asynchronous default operations.
fn default_async_properties() -> FileSystemProperties {
    FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("default-async")
                .expect("test provider id should be valid"),
            "default-async",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::List)
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::Write)
            .with_guaranteed(FileSystemCapability::CreateDirectory)
            .with_guaranteed(FileSystemCapability::Delete)
            .with_guaranteed(FileSystemCapability::Rename)
            .with_guaranteed(FileSystemCapability::Copy)
            .with_guaranteed(FileSystemCapability::TempFile)
            .with_guaranteed(FileSystemCapability::TempDirectory),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("default provider properties should be valid")
}

fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}

fn assert_async_spi_object_safe(_: Arc<dyn AsyncFileSystemSpi>) {}

/// Resolves an immediately-ready runtime-neutral test future.
fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(future);
    match Pin::as_mut(&mut future).poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test future must complete immediately"),
    }
}

#[test]
fn test_async_file_system_is_clone_but_not_a_trait_object() {
    let file_system = AsyncFileSystem::from_spi(PropertiesOnlySpi)
        .expect("facade construction should succeed");
    let clone = file_system.clone();
    assert_eq!(
        file_system.properties().info().provider_id(),
        clone.properties().info().provider_id()
    );
    assert_async_spi_object_safe(Arc::new(PropertiesOnlySpi));
}

/// Exercises the optional trait default which explicitly declines a provider
/// native copy primitive before the facade reports missing fallback support.
#[test]
fn test_async_spi_default_copy_declines_without_provider_override() {
    let file_system = AsyncFileSystem::from_spi(PropertiesOnlySpi)
        .expect("facade construction should succeed");
    let mut operation = file_system
        .begin_copy(
            Path::parse("/source").expect("source path should parse"),
            Path::parse("/target").expect("target path should parse"),
            CopyOptions::default(),
        )
        .expect("copy preflight should accept advertised copy capability");
    let error = ready(operation.execute())
        .expect_err("fallback must require reader and writer capabilities");
    assert_eq!(FsErrorKind::UnsupportedCapability, error.error().kind());
}

/// Exercises every optional asynchronous SPI default implementation through
/// the public facade and verifies that each reports unsupported operation.
#[test]
fn test_async_spi_default_operations_report_unsupported() {
    let file_system = AsyncFileSystem::from_spi(DefaultAsyncSpi {
        properties: default_async_properties(),
    })
    .expect("default provider facade should construct");
    let path = Path::parse("/resource").expect("test path should parse");
    let target = Path::parse("/target").expect("test target should parse");

    let error = ready(file_system.list(&path, ListOptions::default()))
        .expect_err("default list implementation must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::List, error.operation());

    let error = ready(file_system.open_reader(&path, ReadOptions::default()))
        .expect_err("default reader implementation must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());

    let error = ready(file_system.open_writer(&path, WriteOptions::default()))
        .expect_err("default writer implementation must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenWriter, error.operation());

    let error = ready(
        file_system.create_directory(&path, CreateDirectoryOptions::default()),
    )
    .expect_err("default directory implementation must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::CreateDir, error.operation());

    let error = ready(file_system.delete_file(&path, DeleteOptions::default()))
        .expect_err("default file deletion must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::Delete, error.operation());

    let error =
        ready(file_system.delete_directory(&path, DeleteOptions::default()))
            .expect_err("default directory deletion must reject the request");
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::Delete, error.operation());

    let error =
        ready(file_system.rename(&path, &target, RenameOptions::default()))
            .expect_err(
                "default rename implementation must reject the request",
            );
    assert_eq!(FsErrorKind::UnsupportedOperation, error.error().kind());
    assert_eq!(FsOperation::Rename, error.error().operation());

    let mut operation = file_system
        .begin_copy(path.clone(), target.clone(), CopyOptions::default())
        .expect("copy preflight should accept advertised copy capability");
    let error = ready(operation.execute())
        .expect_err("default copy implementation must not complete");
    assert_eq!(FsOperation::Copy, error.error().operation());

    let error =
        match ready(file_system.create_temp_file(TempFileOptions::default())) {
            Ok(_) => panic!(
                "default temporary-file implementation must reject the request"
            ),
            Err(error) => error,
        };
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::CreateTemp, error.operation());

    let error = match ready(
        file_system.create_temp_directory(TempDirectoryOptions::default()),
    ) {
        Ok(_) => panic!(
            "default temporary-directory implementation must reject the request"
        ),
        Err(error) => error,
    };
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::CreateTemp, error.operation());
}
