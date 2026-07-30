// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Runtime-neutral asynchronous provider implementation contract.

use crate::{
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileSystemProperties,
    FsResult,
    RenameOutcome,
};

use super::{
    CopyAttempt,
    CopyDeclineReason,
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedAsyncDirectoryStream,
    OpenedAsyncReader,
    OpenedAsyncTempDirectory,
    OpenedAsyncTempFile,
    OpenedAsyncWriter,
    RenameRequest,
    SpiCopyFailure,
    SpiFuture,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};

/// Object-safe asynchronous provider implementation contract.
pub trait AsyncFileSystemSpi: Send + Sync {
    /// Returns one immutable provider property snapshot without asynchronous
    /// I/O.
    fn properties(&self) -> FileSystemProperties;

    /// Asynchronously reads metadata for a validated request.
    fn stat<'a>(
        &'a self,
        request: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>>;

    /// Asynchronously opens a directory stream.
    fn list<'a>(
        &'a self,
        request: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>>;

    /// Asynchronously opens a reader.
    fn open_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>>;

    /// Asynchronously opens a writer.
    fn open_writer<'a>(
        &'a self,
        request: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>>;

    /// Asynchronously creates a directory.
    fn create_directory<'a>(
        &'a self,
        request: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>>;

    /// Asynchronously deletes a file.
    fn delete_file<'a>(
        &'a self,
        request: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>>;

    /// Asynchronously deletes a directory.
    fn delete_directory<'a>(
        &'a self,
        request: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>>;

    /// Attempts an optional native asynchronous copy primitive.
    #[inline]
    fn try_copy<'a>(
        &'a self,
        _request: CopyRequest<'a>,
    ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        Box::pin(async {
            Ok(CopyAttempt::Declined(CopyDeclineReason::NotImplemented))
        })
    }

    /// Asynchronously renames a resource.
    fn rename<'a>(
        &'a self,
        request: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>>;

    /// Asynchronously creates a temporary file.
    fn create_temp_file<'a>(
        &'a self,
        request: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>>;

    /// Asynchronously creates a temporary directory.
    fn create_temp_directory<'a>(
        &'a self,
        request: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>>;
}
