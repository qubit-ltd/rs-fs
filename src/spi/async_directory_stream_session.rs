// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Provider-side asynchronous directory enumeration sessions.

use super::SpiFuture;
use crate::error::FsResult;
use crate::metadata::DirEntry;

/// Provider session underlying a concrete
/// [`crate::directory::AsyncDirectoryStream`].
pub trait AsyncDirectoryStreamSession: Send {
    /// Asynchronously reads the next directory entry or provider page.
    ///
    /// # Returns
    /// A future resolving to one entry or `None` at end of enumeration.
    fn next_entry_async(&mut self)
    -> SpiFuture<'_, FsResult<Option<DirEntry>>>;
}
