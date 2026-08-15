// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary filesystem resource handles and persistence outcomes.

#[cfg(feature = "async")]
mod async_temp_directory;
#[cfg(feature = "async")]
mod async_temp_file;
mod persist_cleanup_state;
mod persist_failure;
mod persist_failure_state;
mod persist_options;
mod persist_outcome;
mod temp_directory;
mod temp_file;
mod temp_options;
mod temp_resource_state;

#[cfg(feature = "async")]
pub use async_temp_directory::AsyncTempDirectory;
#[cfg(feature = "async")]
pub use async_temp_file::AsyncTempFile;
pub use persist_cleanup_state::PersistCleanupState;
pub use persist_failure::PersistFailure;
pub use persist_failure_state::PersistFailureState;
pub use persist_options::PersistOptions;
pub use persist_outcome::PersistOutcome;
pub use temp_directory::TempDirectory;
pub use temp_file::TempFile;
pub use temp_options::TempOptions;
pub use temp_resource_state::TempResourceState;
