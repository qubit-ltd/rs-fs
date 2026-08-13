// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Application-facing file, directory, and writer handles.

#[cfg(feature = "async")]
mod async_directory_stream;
#[cfg(feature = "async")]
mod async_file_reader;
#[cfg(feature = "async")]
mod async_file_writer;
#[cfg(feature = "async")]
mod async_write_all_failure;
mod directory_entry_validation;
mod directory_stream;
mod directory_stream_state;
mod file_reader;
mod file_writer;
mod write_abort_outcome;
mod write_all_failure;
mod write_failure;
mod write_failure_state;
mod writer_state;

#[cfg(feature = "async")]
pub use async_directory_stream::AsyncDirectoryStream;
#[cfg(feature = "async")]
pub use async_file_reader::AsyncFileReader;
#[cfg(feature = "async")]
pub use async_file_writer::AsyncFileWriter;
#[cfg(feature = "async")]
pub use async_write_all_failure::AsyncWriteAllFailure;
pub use directory_stream::DirectoryStream;
pub use directory_stream_state::DirectoryStreamState;
pub use file_reader::FileReader;
pub use file_writer::FileWriter;
pub use write_abort_outcome::WriteAbortOutcome;
pub use write_all_failure::WriteAllFailure;
pub use write_failure::WriteFailure;
pub use write_failure_state::WriteFailureState;
pub use writer_state::WriterState;
