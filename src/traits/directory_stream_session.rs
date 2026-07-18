// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Provider-side synchronous directory enumeration sessions.

use crate::{
    DirEntry,
    FsResult,
};

/// Provider session underlying a concrete [`crate::DirectoryStream`].
pub trait DirectoryStreamSession: Send {
    /// Reads the next directory entry.
    ///
    /// # Returns
    /// `Some` for one entry or `None` at the end of enumeration.
    ///
    /// # Errors
    /// Returns a filesystem error when a later page cannot be loaded.
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>>;
}
