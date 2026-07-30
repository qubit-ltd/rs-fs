// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-side synchronous directory enumeration sessions.

use crate::{
    DirEntry,
    FsResult,
};

/// Provider directory enumeration session.
pub trait DirectoryStreamSpi: Send {
    /// Returns the next lazy directory entry.
    ///
    /// # Returns
    /// `Some(entry)` for the next item or `None` after enumeration completes.
    ///
    /// # Errors
    /// Returns a provider enumeration failure with filesystem context.
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>>;
}
