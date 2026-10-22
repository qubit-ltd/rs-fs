// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Observable lifecycle state for an owning asynchronous copy operation.

use crate::copy::CopyFailureState;

/// Stable lifecycle state for [`crate::copy::AsyncCopyOperation`].
///
/// # Examples
///
/// This example observes an operation before and after execution.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
/// # poll_support::ready(async {
/// use qubit_fs::Path;
/// use qubit_fs::copy::AsyncCopyOperationState;
/// use qubit_fs::copy::CopyOptions;
///
/// let mut operation = filesystem.begin_copy(
///     Path::parse("/source")?, Path::parse("/target")?, CopyOptions::default(),
/// )?;
/// assert_eq!(AsyncCopyOperationState::Ready, operation.state());
/// operation.execute().await?;
/// assert_eq!(AsyncCopyOperationState::Completed, operation.state());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AsyncCopyOperationState {
    /// The operation passed synchronous preflight and has not been polled.
    Ready,
    /// Provider I/O may be in progress.
    Running,
    /// A copy outcome was returned successfully.
    Completed,
    /// The operation failed with its confirmed publication state.
    Failed(
        /// Confirmed destination publication state at failure time.
        CopyFailureState,
    ),
}
