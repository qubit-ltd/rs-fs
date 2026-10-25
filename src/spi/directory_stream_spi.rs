// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Provider-side synchronous directory enumeration sessions.

use crate::error::FsResult;
use crate::metadata::DirEntry;

/// Provider directory enumeration session.
///
/// # Examples
///
/// ```
/// use qubit_fs::error::FsResult;
/// use qubit_fs::metadata::DirEntry;
/// use qubit_fs::spi::DirectoryStreamSpi;
///
/// struct EmptyStream;
/// impl DirectoryStreamSpi for EmptyStream {
///     fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
///         Ok(None)
///     }
/// }
///
/// let mut stream = EmptyStream;
/// assert!(stream.next_entry().expect("the provider cannot fail").is_none());
/// ```
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
