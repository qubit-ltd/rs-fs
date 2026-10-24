// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-side asynchronous directory enumeration sessions.

use super::SpiFuture;
use crate::error::FsResult;
use crate::metadata::DirEntry;

/// Provider session underlying a concrete
/// [`crate::directory::AsyncDirectoryStream`].
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::AsyncDirectoryStreamSession;
///
/// async fn read_next<T: AsyncDirectoryStreamSession>(session: &mut T) {
///     let result = session.next_entry_async().await;
///     assert!(result.is_ok());
/// }
/// ```
pub trait AsyncDirectoryStreamSession: Send {
    /// Asynchronously reads the next directory entry or provider page.
    ///
    /// # Returns
    /// A future resolving to the next entry, or `None` when enumeration is
    /// complete. Providers return [`FsResult::Err`] when enumeration fails;
    /// the error retains the filesystem context available to the provider.
    ///
    /// # Errors
    /// The future resolves to an error when the provider cannot continue
    /// enumeration.
    fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>>;
}
