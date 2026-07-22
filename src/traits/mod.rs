// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Core filesystem traits.

mod async_directory_stream;
mod async_directory_stream_ext;
mod async_directory_stream_session;
mod async_file_reader;
mod async_file_system;
mod async_file_system_ext;
mod async_file_write_session;
mod async_file_writer;
mod directory_stream;
mod directory_stream_ext;
mod directory_stream_session;
mod file_reader;
mod file_system;
mod file_system_ext;
mod file_system_properties;
mod file_write_session;
mod file_writer;
mod fs_future;
mod write_failure;
mod write_failure_state;
mod writer_state;

pub use async_directory_stream::AsyncDirectoryStream;
pub use async_directory_stream_ext::AsyncDirectoryStreamExt;
pub use async_directory_stream_session::AsyncDirectoryStreamSession;
pub use async_file_reader::AsyncFileReader;
pub use async_file_system::AsyncFileSystem;
pub use async_file_system_ext::AsyncFileSystemExt;
pub use async_file_write_session::AsyncFileWriteSession;
pub use async_file_writer::AsyncFileWriter;
pub use directory_stream::DirectoryStream;
pub use directory_stream_ext::DirectoryStreamExt;
pub use directory_stream_session::DirectoryStreamSession;
pub use file_reader::FileReader;
pub use file_system::FileSystem;
pub use file_system_ext::FileSystemExt;
pub use file_system_properties::FileSystemProperties;
pub use file_write_session::FileWriteSession;
pub use file_writer::FileWriter;
pub use fs_future::FsFuture;
pub use write_failure::WriteFailure;
pub use write_failure_state::WriteFailureState;
pub use writer_state::WriterState;
