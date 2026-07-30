// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous provider implementation contract.

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
    OpenedDirectoryStream,
    OpenedReader,
    OpenedTempDirectory,
    OpenedTempFile,
    OpenedWriter,
    RenameRequest,
    SpiCopyFailure,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use crate::{
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileSystemProperties,
    FsResult,
    RenameOutcome,
};

/// Synchronous provider implementation contract.
pub trait FileSystemSpi: Send + Sync {
    /// Returns one immutable property snapshot.
    fn properties(&self) -> FileSystemProperties;
    /// Reads metadata for a validated request.
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse>;
    /// Opens a directory stream.
    fn list(&self, request: ListRequest<'_>)
    -> FsResult<OpenedDirectoryStream>;
    /// Opens a reader.
    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader>;
    /// Opens a writer.
    fn open_writer(
        &self,
        request: OpenWriterRequest<'_>,
    ) -> FsResult<OpenedWriter>;
    /// Creates a directory.
    fn create_directory(
        &self,
        request: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome>;
    /// Deletes a file.
    fn delete_file(
        &self,
        request: DeleteFileRequest<'_>,
    ) -> FsResult<DeleteOutcome>;
    /// Deletes a directory.
    fn delete_directory(
        &self,
        request: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome>;
    /// Attempts an optional provider copy primitive.
    #[inline(always)]
    fn try_copy(
        &self,
        _request: CopyRequest<'_>,
    ) -> Result<CopyAttempt, SpiCopyFailure> {
        Ok(CopyAttempt::Declined(CopyDeclineReason::NotImplemented))
    }
    /// Renames a resource.
    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure>;
    /// Creates a temporary file.
    fn create_temp_file(
        &self,
        request: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile>;
    /// Creates a temporary directory.
    fn create_temp_directory(
        &self,
        request: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory>;
}
