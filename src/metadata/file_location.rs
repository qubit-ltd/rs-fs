// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stable location captured when a file handle is opened.

use crate::{FileSystemId, FsPath, FsUri};

/// Provider-local identity of a file captured when a handle is opened.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileLocation {
    file_system_id: FileSystemId,
    path: FsPath,
    uri: Option<FsUri>,
}

impl FileLocation {
    /// Creates a location from a configured filesystem and provider-local path.
    ///
    /// # Parameters
    /// - `file_system_id`: Stable identity of the configured filesystem.
    /// - `path`: Provider-local path at open time.
    ///
    /// # Returns
    /// A location without a reconstructed URI.
    #[inline]
    #[must_use]
    pub fn new(file_system_id: FileSystemId, path: FsPath) -> Self {
        Self {
            file_system_id,
            path,
            uri: None,
        }
    }

    /// Attaches a canonical, credential-free URI.
    ///
    /// # Parameters
    /// - `uri`: Safe resource URI from registry resolution.
    ///
    /// # Returns
    /// The updated location.
    #[inline]
    #[must_use]
    pub fn with_uri(mut self, uri: FsUri) -> Self {
        self.uri = Some(uri);
        self
    }

    /// Returns the configured filesystem identity.
    ///
    /// # Returns
    /// The stable filesystem id.
    #[inline]
    #[must_use]
    pub fn file_system_id(&self) -> &FileSystemId {
        &self.file_system_id
    }

    /// Returns the provider-local path captured at open time.
    ///
    /// The value is a snapshot and is not rewritten after a rename.
    ///
    /// # Returns
    /// The opened path.
    #[inline]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        &self.path
    }

    /// Returns a canonical credential-free URI when safely reconstructable.
    ///
    /// # Returns
    /// The optional resolved URI.
    #[inline]
    #[must_use]
    pub fn uri(&self) -> Option<&FsUri> {
        self.uri.as_ref()
    }
}
