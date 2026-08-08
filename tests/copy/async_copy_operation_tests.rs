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

use qubit_fs::AsyncCopyOperationState;
use qubit_fs::AsyncFileSystem;
use qubit_fs::CopyOptions;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOutcome;
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
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::CopyAttempt;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiCopyFailure;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;

struct CopySpi;

impl AsyncFileSystemSpi for CopySpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("copy-test")
                    .expect("test id should be valid"),
                "copy-test",
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

    fn try_copy<'a>(
        &'a self,
        _: qubit_fs::spi::CopyRequest<'a>,
    ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        Box::pin(pending())
    }
}

fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}

#[test]
fn test_begin_copy_only_runs_synchronous_preflight() {
    let file_system =
        AsyncFileSystem::from_spi(CopySpi).expect("facade should construct");
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
    let file_system =
        AsyncFileSystem::from_spi(CopySpi).expect("facade should construct");
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
        AsyncCopyOperationState::Failed(
            qubit_fs::CopyFailureState::Indeterminate
        )
    );
    assert!(!operation.has_recovery_writer());
}
