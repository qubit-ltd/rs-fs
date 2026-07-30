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
///
/// Operation futures perform provider I/O only while polled. Dropping a
/// pending future cancels local polling but does not imply that remote work was
/// rolled back; mutation failures must preserve confirmed progress in their
/// typed failure state.
pub trait AsyncFileSystemSpi: Send + Sync {
    /// Returns one immutable provider property snapshot without asynchronous
    /// I/O.
    ///
    /// # Returns
    /// The provider's immutable property snapshot.
    fn properties(&self) -> FileSystemProperties;

    /// Asynchronously reads metadata for a validated request.
    ///
    /// # Parameters
    /// - `request`: Facade-validated stat request.
    ///
    /// # Returns
    /// A future resolving to path-bound metadata.
    ///
    /// # Errors
    /// Resolves to the provider lookup failure with filesystem context.
    fn stat<'a>(
        &'a self,
        request: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>>;

    /// Asynchronously opens a directory stream.
    ///
    /// # Parameters
    /// - `request`: Facade-validated list request.
    ///
    /// # Returns
    /// A future resolving to an opened enumeration session.
    ///
    /// # Errors
    /// Resolves to the provider open failure with filesystem context.
    fn list<'a>(
        &'a self,
        request: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>>;

    /// Asynchronously opens a reader.
    ///
    /// # Parameters
    /// - `request`: Facade-validated reader request.
    ///
    /// # Returns
    /// A future resolving to an identity-bound reader.
    ///
    /// # Errors
    /// Resolves to the provider open failure with filesystem context.
    fn open_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>>;

    /// Asynchronously opens a writer.
    ///
    /// # Parameters
    /// - `request`: Facade-validated writer request.
    ///
    /// # Returns
    /// A future resolving to an identity-bound writer.
    ///
    /// # Errors
    /// Resolves to the provider open failure with filesystem context.
    fn open_writer<'a>(
        &'a self,
        request: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>>;

    /// Asynchronously creates a directory.
    ///
    /// # Parameters
    /// - `request`: Facade-validated directory-creation request.
    ///
    /// # Returns
    /// A future resolving to the confirmed creation outcome.
    ///
    /// # Errors
    /// Resolves to the provider creation failure with filesystem context.
    fn create_directory<'a>(
        &'a self,
        request: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>>;

    /// Asynchronously deletes a file.
    ///
    /// # Parameters
    /// - `request`: Facade-validated file-deletion request.
    ///
    /// # Returns
    /// A future resolving to the confirmed deletion outcome.
    ///
    /// # Errors
    /// Resolves to the provider deletion failure with filesystem context.
    fn delete_file<'a>(
        &'a self,
        request: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>>;

    /// Asynchronously deletes a directory.
    ///
    /// # Parameters
    /// - `request`: Facade-validated directory-deletion request.
    ///
    /// # Returns
    /// A future resolving to the confirmed deletion outcome.
    ///
    /// # Errors
    /// Resolves to the provider deletion failure with filesystem context.
    fn delete_directory<'a>(
        &'a self,
        request: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>>;

    /// Attempts an optional native asynchronous copy primitive.
    ///
    /// # Parameters
    /// - `_request`: Facade-validated copy request.
    ///
    /// # Returns
    /// A future resolving to a completed outcome or a typed decline reason.
    ///
    /// # Errors
    /// Resolves to a typed failure preserving confirmed publication progress.
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
    ///
    /// # Parameters
    /// - `request`: Facade-validated rename request.
    ///
    /// # Returns
    /// A future resolving to the confirmed rename outcome.
    ///
    /// # Errors
    /// Resolves to a typed failure preserving confirmed rename progress.
    fn rename<'a>(
        &'a self,
        request: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>>;

    /// Asynchronously creates a temporary file.
    ///
    /// # Parameters
    /// - `request`: Validated temporary-file creation request.
    ///
    /// # Returns
    /// A future resolving to an identity-bound temporary-file session.
    ///
    /// # Errors
    /// Resolves to the provider creation failure with filesystem context.
    fn create_temp_file<'a>(
        &'a self,
        request: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>>;

    /// Asynchronously creates a temporary directory.
    ///
    /// # Parameters
    /// - `request`: Validated temporary-directory creation request.
    ///
    /// # Returns
    /// A future resolving to an identity-bound temporary-directory session.
    ///
    /// # Errors
    /// Resolves to the provider creation failure with filesystem context.
    fn create_temp_directory<'a>(
        &'a self,
        request: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>>;
}
