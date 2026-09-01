// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External lifecycle coverage for owning asynchronous copy operations.

use std::future::Future;
use std::future::pending;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::AsyncCopyOperationState;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyOptions;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOutcome;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::rename::RenameFailureState;
use qubit_fs::rename::RenameOutcome;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::CopyAttempt;
use qubit_fs::spi::CopyRequest;
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
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiCopyFailure;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

struct CopySpi;

impl AsyncFileSystemSpi for CopySpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("copy-test").expect("test id should be valid"),
                "copy-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter)
                .with(ProviderOperation::TryCopy),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Copy),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { Err(unused()) })
    }
    fn list<'a>(&'a self, _: ListRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        Box::pin(async { Err(unused()) })
    }
    fn open_reader<'a>(&'a self, _: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async { Err(unused()) })
    }
    fn open_writer<'a>(&'a self, _: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        Box::pin(async { Err(unused()) })
    }
    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_file<'a>(&'a self, _: DeleteFileRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_directory<'a>(&'a self, _: DeleteDirectoryRequest<'a>) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn rename<'a>(&'a self, _: RenameRequest<'a>) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        Box::pin(async { Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged)) })
    }
    fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async { Err(unused()) })
    }
    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async { Err(unused()) })
    }

    fn try_copy<'a>(&'a self, _: CopyRequest<'a>) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        Box::pin(pending())
    }
}

fn unused() -> FsError {
    FsError::new(FsErrorKind::UnsupportedOperation, FsOperation::Other, "unused")
}

#[test]
fn test_begin_copy_only_runs_synchronous_preflight() {
    let file_system = AsyncFileSystem::from_spi(CopySpi).expect("facade should construct");
    let operation = file_system
        .begin_copy(
            Path::parse("/source").expect("source path should parse"),
            Path::parse("/target").expect("target path should parse"),
            CopyOptions::default(),
        )
        .expect("preflight should succeed");
    assert_eq!(operation.state(), AsyncCopyOperationState::Ready);
}

#[test]
fn test_dropping_polled_execute_future_marks_operation_indeterminate() {
    let file_system = AsyncFileSystem::from_spi(CopySpi).expect("facade should construct");
    let mut operation = file_system
        .begin_copy(
            Path::parse("/source").expect("source path should parse"),
            Path::parse("/target").expect("target path should parse"),
            CopyOptions::default(),
        )
        .expect("preflight should succeed");
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    let mut future = Box::pin(operation.execute());
    assert!(matches!(future.as_mut().poll(&mut context), Poll::Pending));
    drop(future);
    assert_eq!(
        operation.state(),
        AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate)
    );
    assert!(!operation.has_recovery_writer());
}
