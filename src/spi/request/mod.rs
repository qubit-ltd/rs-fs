// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Unforgeable requests passed from the facade to providers.
//!
//! ```compile_fail
//! use qubit_fs::{Path, spi::StatRequest};
//!
//! let path = Path::root();
//! let _ = StatRequest::new(&path);
//! ```

mod copy_request;
mod create_directory_request;
mod create_temp_directory_request;
mod create_temp_file_request;
mod delete_directory_request;
mod delete_file_request;
mod internal;
mod list_request;
mod open_reader_request;
mod open_writer_request;
mod persist_request;
mod rename_request;
mod stat_request;

pub use copy_request::CopyRequest;
pub use create_directory_request::CreateDirectoryRequest;
pub use create_temp_directory_request::CreateTempDirectoryRequest;
pub use create_temp_file_request::CreateTempFileRequest;
pub use delete_directory_request::DeleteDirectoryRequest;
pub use delete_file_request::DeleteFileRequest;
pub use list_request::ListRequest;
pub use open_reader_request::OpenReaderRequest;
pub use open_writer_request::OpenWriterRequest;
pub use persist_request::PersistRequest;
pub use rename_request::RenameRequest;
pub use stat_request::StatRequest;
