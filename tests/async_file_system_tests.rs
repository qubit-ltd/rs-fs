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
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::spi::{
    AsyncFileSystemSpi,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    RenameRequest,
    SpiFuture,
    StatRequest,
};
use qubit_fs::{
    AsyncFileSystem,
    CopyOptions,
    CreateDirectoryOutcome,
    DeleteOutcome,
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
    PathSemantics,
    RenameFailureState,
    RenameOutcome,
    SymlinkPolicy,
};

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
                .with(qubit_fs::FileSystemCapability::Copy),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }

    fn stat<'a>(
        &'a self,
        _: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::StatResponse>> {
        Box::pin(async { Err(unused()) })
    }
    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncDirectoryStream>>
    {
        Box::pin(async { Err(unused()) })
    }
    fn open_reader<'a>(
        &'a self,
        _: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }
    fn open_writer<'a>(
        &'a self,
        _: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncWriter>> {
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
    ) -> SpiFuture<'a, Result<RenameOutcome, qubit_fs::spi::SpiRenameFailure>>
    {
        Box::pin(async {
            Err(qubit_fs::spi::SpiRenameFailure::new(
                unused(),
                RenameFailureState::Unchanged,
            ))
        })
    }
    fn create_temp_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }
    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<qubit_fs::spi::OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }
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
