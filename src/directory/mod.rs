// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory options, streams, outcomes, and facade operation objects.

#[cfg(feature = "async")]
mod async_directory_operation;
#[cfg(feature = "async")]
mod async_directory_stream;
mod create_directory_options;
mod create_directory_outcome;
mod delete_options;
mod delete_outcome;
mod directory_entry_validation;
mod directory_operation;
mod directory_stream;
mod directory_stream_state;
mod internal;
mod list_filter;
mod list_options;

#[cfg(feature = "async")]
pub(crate) use async_directory_operation::AsyncDirectoryOperation;
#[cfg(feature = "async")]
pub use async_directory_stream::AsyncDirectoryStream;
pub use create_directory_options::CreateDirectoryOptions;
pub use create_directory_outcome::CreateDirectoryOutcome;
pub use delete_options::DeleteOptions;
pub use delete_outcome::DeleteOutcome;
pub(crate) use directory_operation::DirectoryOperation;
pub use directory_stream::DirectoryStream;
pub use directory_stream_state::DirectoryStreamState;
pub use list_filter::ListFilter;
pub use list_options::ListOptions;
