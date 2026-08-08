// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider service-provider interface; application code uses
//! [`crate::FileSystem`].

#[cfg(feature = "async")]
mod async_directory_stream_session;
#[cfg(feature = "async")]
mod async_file_system_spi;
#[cfg(feature = "async")]
mod async_file_write_session;
#[cfg(feature = "async")]
mod async_temp_resource_spi;
mod copy_attempt;
mod copy_decline_reason;
mod directory_stream_spi;
mod file_system_spi;
mod file_writer_spi;
mod internal;
#[cfg(feature = "async")]
mod opened_async_directory_stream;
#[cfg(feature = "async")]
mod opened_async_reader;
#[cfg(feature = "async")]
mod opened_async_temp_directory;
#[cfg(feature = "async")]
mod opened_async_temp_file;
#[cfg(feature = "async")]
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
#[cfg(feature = "async")]
mod spi_future;
mod spi_persist_failure;
mod spi_rename_failure;
mod spi_write_failure;
mod stat_response;
mod temp_resource_spi;

#[cfg(feature = "async")]
pub use async_directory_stream_session::AsyncDirectoryStreamSession;
#[cfg(feature = "async")]
pub use async_file_system_spi::AsyncFileSystemSpi;
#[cfg(feature = "async")]
pub use async_file_write_session::AsyncFileWriteSession;
#[cfg(feature = "async")]
pub use async_temp_resource_spi::AsyncTempResourceSpi;
pub use copy_attempt::CopyAttempt;
pub use copy_decline_reason::CopyDeclineReason;
pub use directory_stream_spi::DirectoryStreamSpi;
pub use file_system_spi::FileSystemSpi;
pub use file_writer_spi::FileWriterSpi;
#[cfg(feature = "async")]
pub use opened_async_directory_stream::OpenedAsyncDirectoryStream;
#[cfg(feature = "async")]
pub use opened_async_reader::OpenedAsyncReader;
#[cfg(feature = "async")]
pub use opened_async_temp_directory::OpenedAsyncTempDirectory;
#[cfg(feature = "async")]
pub use opened_async_temp_file::OpenedAsyncTempFile;
#[cfg(feature = "async")]
pub use opened_async_writer::OpenedAsyncWriter;
pub use opened_directory_stream::OpenedDirectoryStream;
pub use opened_reader::OpenedReader;
pub use opened_temp_directory::OpenedTempDirectory;
pub use opened_temp_file::OpenedTempFile;
pub use opened_writer::OpenedWriter;
pub use request::CopyRequest;
pub use request::CreateDirectoryRequest;
pub use request::CreateTempDirectoryRequest;
pub use request::CreateTempFileRequest;
pub use request::DeleteDirectoryRequest;
pub use request::DeleteFileRequest;
pub use request::ListRequest;
pub use request::OpenReaderRequest;
pub use request::OpenWriterRequest;
pub use request::PersistRequest;
pub use request::RenameRequest;
pub use request::StatRequest;
pub use resolved_copy_options::ResolvedCopyOptions;
pub use resolved_create_directory_options::ResolvedCreateDirectoryOptions;
pub use resolved_delete_options::ResolvedDeleteOptions;
pub use resolved_list_options::ResolvedListOptions;
pub use resolved_read_options::ResolvedReadOptions;
pub use resolved_rename_options::ResolvedRenameOptions;
pub use resolved_write_options::ResolvedWriteOptions;
pub use spi_copy_failure::SpiCopyFailure;
#[cfg(feature = "async")]
pub use spi_future::SpiFuture;
pub use spi_persist_failure::SpiPersistFailure;
pub use spi_rename_failure::SpiRenameFailure;
pub use spi_write_failure::SpiWriteFailure;
pub use stat_response::StatResponse;
pub use temp_resource_spi::TempResourceSpi;
