/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Directory listing stream trait.

use std::fmt::Debug;

use crate::{
    DirEntry,
    FsResult,
};

/// Stream of directory entries.
pub trait DirectoryStream: Debug + Send {
    /// Reads the next directory entry.
    ///
    /// # Returns
    /// `Ok(Some(entry))` for an entry, `Ok(None)` at end of stream.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot continue listing.
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>>;
}
