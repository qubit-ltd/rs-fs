// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Error type for observed asynchronous aggregate reads.

use std::error::Error;
use std::fmt;

use crate::error::FsError;

/// Error returned by [`AsyncFileSystem::read_all_observed`] and
/// [`AsyncFileSystem::read_prefix_observed`].
///
/// [`AsyncFileSystem::read_all_observed`]: crate::AsyncFileSystem::read_all_observed
/// [`AsyncFileSystem::read_prefix_observed`]: crate::AsyncFileSystem::read_prefix_observed
#[derive(Debug)]
pub enum ObservedReadError {
    /// The observer stopped the read before its next provider operation.
    Stopped,
    /// The provider or aggregate-read policy returned a filesystem error.
    Provider(
        /// Original filesystem error and its existing context.
        FsError,
    ),
}

impl fmt::Display for ObservedReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stopped => formatter.write_str("observed read stopped by observer"),
            Self::Provider(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ObservedReadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Stopped => None,
            Self::Provider(error) => Some(error),
        }
    }
}

/// Converts an underlying filesystem error into an observed-read error.
impl From<FsError> for ObservedReadError {
    /// Wraps the filesystem error without changing its type or context.
    ///
    /// # Parameters
    /// - `error`: Filesystem error returned by validation, a provider, or
    ///   buffering.
    ///
    /// # Returns
    /// An observed-read error carrying the original filesystem error.
    #[inline]
    fn from(error: FsError) -> Self {
        Self::Provider(error)
    }
}
