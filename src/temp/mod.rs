// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary filesystem resource handles and persistence outcomes.

mod async_temp_dir;
mod async_temp_file;
mod async_temp_resource_session;
mod persist_failure;
mod persist_failure_state;
mod persist_future;
mod persist_outcome;
mod temp_dir;
mod temp_dir_options;
mod temp_file;
mod temp_file_options;
mod temp_resource_session;
mod temp_resource_state;

pub use async_temp_dir::AsyncTempDir;
pub use async_temp_file::AsyncTempFile;
pub use async_temp_resource_session::AsyncTempResourceSession;
pub use persist_failure::PersistFailure;
pub use persist_failure_state::PersistFailureState;
pub use persist_future::PersistFuture;
pub use persist_outcome::PersistOutcome;
pub use temp_dir::TempDir;
pub use temp_dir_options::TempDirOptions;
pub use temp_file::TempFile;
pub use temp_file_options::TempFileOptions;
pub use temp_resource_session::TempResourceSession;
pub use temp_resource_state::TempResourceState;
