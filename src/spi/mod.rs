// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider service-provider interface; application code uses
//! [`crate::FileSystem`].

mod async_directory_stream_session;
mod async_file_system_spi;
mod async_file_write_session;
mod async_temp_resource_spi;
mod copy_attempt;
mod copy_decline_reason;
mod directory_stream_spi;
mod file_system_spi;
mod file_writer_spi;
mod internal;
mod opened_async_directory_stream;
mod opened_async_reader;
mod opened_async_temp_directory;
mod opened_async_temp_file;
mod opened_async_writer;
mod opened_directory_stream;
mod opened_reader;
mod opened_temp_directory;
mod opened_temp_file;
mod opened_writer;
mod request;
mod resolved_copy_options;
mod resolved_create_directory_options;
mod resolved_delete_options;
mod resolved_list_options;
mod resolved_read_options;
mod resolved_rename_options;
mod resolved_write_options;
mod spi_copy_failure;
mod spi_future;
mod spi_persist_failure;
mod spi_rename_failure;
mod spi_write_failure;
mod stat_response;
mod temp_resource_spi;

pub use async_directory_stream_session::AsyncDirectoryStreamSession;
pub use async_file_system_spi::AsyncFileSystemSpi;
pub use async_file_write_session::AsyncFileWriteSession;
pub use async_temp_resource_spi::AsyncTempResourceSpi;
pub use copy_attempt::CopyAttempt;
pub use copy_decline_reason::CopyDeclineReason;
pub use directory_stream_spi::DirectoryStreamSpi;
pub use file_system_spi::FileSystemSpi;
pub use file_writer_spi::FileWriterSpi;
pub use opened_async_directory_stream::OpenedAsyncDirectoryStream;
pub use opened_async_reader::OpenedAsyncReader;
pub use opened_async_temp_directory::OpenedAsyncTempDirectory;
pub use opened_async_temp_file::OpenedAsyncTempFile;
pub use opened_async_writer::OpenedAsyncWriter;
pub use opened_directory_stream::OpenedDirectoryStream;
pub use opened_reader::OpenedReader;
pub use opened_temp_directory::OpenedTempDirectory;
pub use opened_temp_file::OpenedTempFile;
pub use opened_writer::OpenedWriter;
pub use request::{
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    PersistRequest,
    RenameRequest,
    StatRequest,
};
pub use resolved_copy_options::ResolvedCopyOptions;
pub use resolved_create_directory_options::ResolvedCreateDirectoryOptions;
pub use resolved_delete_options::ResolvedDeleteOptions;
pub use resolved_list_options::ResolvedListOptions;
pub use resolved_read_options::ResolvedReadOptions;
pub use resolved_rename_options::ResolvedRenameOptions;
pub use resolved_write_options::ResolvedWriteOptions;
pub use spi_copy_failure::SpiCopyFailure;
pub use spi_future::SpiFuture;
pub use spi_persist_failure::SpiPersistFailure;
pub use spi_rename_failure::SpiRenameFailure;
pub use spi_write_failure::SpiWriteFailure;
pub use stat_response::StatResponse;
pub use temp_resource_spi::TempResourceSpi;
