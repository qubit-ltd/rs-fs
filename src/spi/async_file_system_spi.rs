// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Runtime-neutral asynchronous provider contract and opened-handle envelopes.

use std::future::Future;
use std::pin::Pin;

use crate::{
    AsyncDirectoryStream, AsyncFileReader, AsyncFileWriter, AtomicityRequirement, CopyFailureState,
    CopyStats, CreateDirectoryOutcome, DeleteOutcome, FileMetadata, FileSystemProperties, FsError,
    FsResult, OpenedFileInfo, RenameFailureState, RenameOutcome,
};

use super::{
    AsyncDirectoryStreamSession, AsyncFileWriteSession, CopyAttempt, CopyDeclineReason,
    CopyRequest, CreateDirectoryRequest, CreateTempDirectoryRequest, CreateTempFileRequest,
    DeleteDirectoryRequest, DeleteFileRequest, ListRequest, OpenReaderRequest, OpenWriterRequest,
    RenameRequest, SpiCopyFailure, SpiRenameFailure, StatRequest, StatResponse,
};

/// Runtime-neutral boxed future used by asynchronous providers.
pub type SpiFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An already-open asynchronous reader bound to provider identity.
pub struct OpenedAsyncReader {
    info: OpenedFileInfo,
    reader: Box<dyn qubit_io::AsyncInput<Item = u8> + Send>,
}

impl OpenedAsyncReader {
    /// Wraps a provider-opened reader session and its claimed identity.
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        reader: Box<dyn qubit_io::AsyncInput<Item = u8> + Send>,
    ) -> Self {
        Self { info, reader }
    }

    /// Returns the immutable provider-opened identity.
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated reader into the facade handle.
    pub(crate) fn into_reader(self) -> AsyncFileReader {
        AsyncFileReader::new(self.info, self.reader)
    }
}

/// An already-open asynchronous writer bound to provider identity.
pub struct OpenedAsyncWriter {
    info: OpenedFileInfo,
    session: Box<dyn AsyncFileWriteSession>,
}

impl OpenedAsyncWriter {
    /// Wraps an opened provider writer session and its validated identity.
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncFileWriteSession>) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    #[must_use]
    pub fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the validated writer into the facade handle.
    pub(crate) fn into_writer(
        self,
        atomicity: AtomicityRequirement,
        provider: &str,
        max_write_bytes: Option<u64>,
    ) -> AsyncFileWriter {
        AsyncFileWriter::new(
            self.info,
            self.session,
            atomicity,
            provider,
            max_write_bytes,
        )
    }
}

/// An already-open asynchronous directory stream.
pub struct OpenedAsyncDirectoryStream {
    session: Box<dyn AsyncDirectoryStreamSession>,
}

impl OpenedAsyncDirectoryStream {
    /// Wraps an opened provider directory-enumeration session.
    #[must_use]
    pub fn new(session: Box<dyn AsyncDirectoryStreamSession>) -> Self {
        Self { session }
    }

    /// Transfers the stream into the facade handle.
    pub(crate) fn into_stream(
        self,
        root: crate::Path,
        options: crate::ListOptions,
        provider: &str,
    ) -> AsyncDirectoryStream {
        AsyncDirectoryStream::new(root, self.session, options, provider)
    }
}

/// An already-created asynchronous temporary-file handle.
pub struct OpenedAsyncTempFile {
    info: OpenedFileInfo,
    session: Box<dyn AsyncTempResourceSpi>,
}

impl OpenedAsyncTempFile {
    /// Wraps an asynchronous temporary-file handle.
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncTempResourceSpi>) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    #[must_use]
    pub const fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the provider session into the facade handle.
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn AsyncTempResourceSpi>) {
        (self.info, self.session)
    }
}

/// An already-created asynchronous temporary-directory handle.
pub struct OpenedAsyncTempDirectory {
    info: OpenedFileInfo,
    session: Box<dyn AsyncTempResourceSpi>,
}

impl OpenedAsyncTempDirectory {
    /// Wraps an asynchronous temporary-directory handle.
    #[must_use]
    pub fn new(info: OpenedFileInfo, session: Box<dyn AsyncTempResourceSpi>) -> Self {
        Self { info, session }
    }

    /// Returns the immutable provider-opened identity.
    #[must_use]
    pub const fn info(&self) -> &OpenedFileInfo {
        &self.info
    }

    /// Transfers the provider session into the facade handle.
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn AsyncTempResourceSpi>) {
        (self.info, self.session)
    }
}

/// Provider-side asynchronous temporary-resource lifecycle session.
pub trait AsyncTempResourceSpi: Send {
    /// Asynchronously confirms provider cleanup.
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>>;

    /// Asynchronously releases caller cleanup responsibility.
    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>>;

    /// Asynchronously persists this resource to a validated target.
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: super::PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<crate::PersistOutcome, super::SpiPersistFailure>>;
}

/// Object-safe asynchronous provider implementation contract.
pub trait AsyncFileSystemSpi: Send + Sync {
    /// Returns one immutable provider property snapshot without asynchronous
    /// I/O.
    fn properties(&self) -> FileSystemProperties;

    /// Asynchronously reads metadata for a validated request.
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>>;

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
    fn try_copy<'a>(
        &'a self,
        _request: CopyRequest<'a>,
    ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        Box::pin(async { Ok(CopyAttempt::Declined(CopyDeclineReason::NotImplemented)) })
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

/// Retains imports used by the parallel synchronous failure envelopes.
#[allow(dead_code)]
fn _typed_failure_facts(
    _: CopyFailureState,
    _: CopyStats,
    _: RenameFailureState,
    _: FsError,
    _: FileMetadata,
) {
}
