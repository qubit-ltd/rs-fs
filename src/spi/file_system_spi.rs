// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous provider contract and provider result envelopes.

use super::{
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    RenameRequest,
    StatRequest,
};
use qubit_io::Input;

use super::{
    DirectoryStreamSpi,
    FileWriterSpi,
    TempResourceSpi,
};
use crate::{
    CopyFailureState,
    CopyStats,
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileMetadata,
    FileSystemProperties,
    FsError,
    FsResult,
    OpenedFileInfo,
    Path,
    RenameFailureState,
    RenameOutcome,
};

/// An already-open provider reader.
pub struct OpenedReader {
    info: OpenedFileInfo,
    reader: Box<dyn Input<Item = u8> + Send>,
}
impl OpenedReader {
    /// Wraps a reader which the provider has fully opened.
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        reader: Box<dyn Input<Item = u8> + Send>,
    ) -> Self {
        Self { info, reader }
    }
    /// Returns the opened reader to the facade.
    pub(crate) fn into_parts(
        self,
    ) -> (OpenedFileInfo, Box<dyn Input<Item = u8> + Send>) {
        (self.info, self.reader)
    }
}
/// An already-open provider writer.
pub struct OpenedWriter {
    info: OpenedFileInfo,
    writer: Box<dyn FileWriterSpi>,
}
impl OpenedWriter {
    /// Wraps a writer which the provider has fully opened.
    #[must_use]
    pub fn new(info: OpenedFileInfo, writer: Box<dyn FileWriterSpi>) -> Self {
        Self { info, writer }
    }
    /// Returns the opened writer to the facade.
    pub(crate) fn into_parts(self) -> (OpenedFileInfo, Box<dyn FileWriterSpi>) {
        (self.info, self.writer)
    }
}
/// An already-open provider directory stream.
pub struct OpenedDirectoryStream {
    stream: Box<dyn DirectoryStreamSpi>,
}
impl OpenedDirectoryStream {
    /// Wraps a directory stream which the provider has fully opened.
    #[must_use]
    pub fn new(stream: Box<dyn DirectoryStreamSpi>) -> Self {
        Self { stream }
    }
    /// Returns the opened stream to the facade.
    pub(crate) fn into_stream(self) -> Box<dyn DirectoryStreamSpi> {
        self.stream
    }
}
/// Provider metadata response bound to the path it describes.
pub struct StatResponse {
    path: Path,
    metadata: FileMetadata,
}
impl StatResponse {
    /// Creates a response for `path` after provider metadata lookup.
    #[must_use]
    pub fn new(path: Path, metadata: FileMetadata) -> Self {
        Self { path, metadata }
    }
    /// Returns the logical path represented by the metadata.
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }
    /// Returns the metadata snapshot.
    #[must_use]
    pub const fn metadata(&self) -> &FileMetadata {
        &self.metadata
    }
    /// Returns the metadata to the validating facade.
    pub(crate) fn into_metadata(self) -> FileMetadata {
        self.metadata
    }
}
/// An already-created provider temporary-file session.
pub struct OpenedTempFile {
    info: OpenedFileInfo,
    session: Box<dyn TempResourceSpi>,
}
impl OpenedTempFile {
    /// Wraps an owned temporary-file session.
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        session: Box<dyn TempResourceSpi>,
    ) -> Self {
        Self { info, session }
    }
    /// Returns the provider-owned parts to the facade.
    pub(crate) fn into_parts(
        self,
    ) -> (OpenedFileInfo, Box<dyn TempResourceSpi>) {
        (self.info, self.session)
    }
}
/// An already-created provider temporary-directory session.
pub struct OpenedTempDirectory {
    info: OpenedFileInfo,
    session: Box<dyn TempResourceSpi>,
}
impl OpenedTempDirectory {
    /// Wraps an owned temporary-directory session.
    #[must_use]
    pub fn new(
        info: OpenedFileInfo,
        session: Box<dyn TempResourceSpi>,
    ) -> Self {
        Self { info, session }
    }
    /// Returns the provider-owned parts to the facade.
    pub(crate) fn into_parts(
        self,
    ) -> (OpenedFileInfo, Box<dyn TempResourceSpi>) {
        (self.info, self.session)
    }
}

/// Reason a provider declined its optional copy fast path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CopyDeclineReason {
    /// The provider does not implement a copy primitive.
    NotImplemented,
    /// The provider primitive cannot safely serve this particular request.
    NotApplicable,
}
/// Optional provider copy result.
#[non_exhaustive]
pub enum CopyAttempt {
    /// A provider completed the copy.
    Completed(crate::CopyOutcome),
    /// The facade may select its later fallback.
    Declined(CopyDeclineReason),
}
/// Typed provider copy failure reserved for copy orchestration.
pub struct SpiCopyFailure {
    error: Box<FsError>,
    state: CopyFailureState,
    partial_stats: CopyStats,
}
impl SpiCopyFailure {
    /// Creates a typed provider copy failure.
    pub fn new(
        error: FsError,
        state: CopyFailureState,
        partial_stats: CopyStats,
    ) -> Self {
        Self {
            error: Box::new(error),
            state,
            partial_stats,
        }
    }
    /// Returns the provider error without exposing a recovery writer.
    pub fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the typed publication state.
    pub const fn state(&self) -> CopyFailureState {
        self.state
    }
    /// Splits this failure into its typed facts.
    pub fn into_parts(self) -> (FsError, CopyFailureState, CopyStats) {
        (*self.error, self.state, self.partial_stats)
    }
}
/// Typed provider rename failure reserved for rename orchestration.
pub struct SpiRenameFailure {
    error: Box<FsError>,
    state: RenameFailureState,
}
impl SpiRenameFailure {
    /// Creates a typed provider rename failure.
    pub fn new(error: FsError, state: RenameFailureState) -> Self {
        Self {
            error: Box::new(error),
            state,
        }
    }
    /// Returns the error.
    pub fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the typed rename state.
    pub const fn state(&self) -> RenameFailureState {
        self.state
    }
    /// Returns the contained error.
    pub fn into_parts(self) -> (FsError, RenameFailureState) {
        (*self.error, self.state)
    }
}

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
