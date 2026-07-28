// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider service-provider interface; application code uses [`crate::FileSystem`].

mod async_directory_stream_session;
mod async_file_system_spi;
mod async_file_write_session;
mod file_system_spi;
mod request;
mod resolved_options;
mod session;

pub use async_directory_stream_session::AsyncDirectoryStreamSession;
pub use async_file_system_spi::{
    AsyncFileSystemSpi, AsyncTempResourceSpi, OpenedAsyncDirectoryStream, OpenedAsyncReader,
    OpenedAsyncTempDirectory, OpenedAsyncTempFile, OpenedAsyncWriter, SpiFuture,
};
pub use async_file_write_session::AsyncFileWriteSession;
pub use file_system_spi::{
    CopyAttempt, CopyDeclineReason, FileSystemSpi, OpenedDirectoryStream, OpenedReader,
    OpenedTempDirectory, OpenedTempFile, OpenedWriter, SpiCopyFailure, SpiRenameFailure,
    StatResponse,
};
pub use request::{
    CopyRequest, CreateDirectoryRequest, CreateTempDirectoryRequest, CreateTempFileRequest,
    DeleteDirectoryRequest, DeleteFileRequest, ListRequest, OpenReaderRequest, OpenWriterRequest,
    PersistRequest, RenameRequest, StatRequest,
};
pub use resolved_options::{
    ResolvedCopyOptions, ResolvedCreateDirectoryOptions, ResolvedDeleteOptions,
    ResolvedListOptions, ResolvedPersistOptions, ResolvedReadOptions, ResolvedRenameOptions,
    ResolvedTempDirectoryOptions, ResolvedTempFileOptions, ResolvedWriteOptions,
};
pub use session::{
    DirectoryStreamSpi, FileWriterSpi, SpiPersistFailure, SpiWriteFailure, TempResourceSpi,
};
