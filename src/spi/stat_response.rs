// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider metadata response.

use crate::{
    FileMetadata,
    Path,
};

/// Provider metadata response bound to the path it describes.
pub struct StatResponse {
    /// Logical path described by the response.
    path: Path,
    /// Provider metadata snapshot for `path`.
    metadata: FileMetadata,
}

impl StatResponse {
    /// Creates a response for `path` after provider metadata lookup.
    ///
    /// # Parameters
    /// - `path`: Logical path described by the metadata.
    /// - `metadata`: Provider metadata snapshot.
    ///
    /// # Returns
    /// A path-bound metadata response.
    #[inline]
    #[must_use]
    pub fn new(path: Path, metadata: FileMetadata) -> Self {
        Self { path, metadata }
    }

    /// Returns the logical path represented by the metadata.
    ///
    /// # Returns
    /// The response path.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the metadata snapshot.
    ///
    /// # Returns
    /// The provider metadata snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn metadata(&self) -> &FileMetadata {
        &self.metadata
    }

    /// Returns the metadata to the validating facade.
    ///
    /// # Returns
    /// The owned provider metadata snapshot.
    #[inline(always)]
    #[must_use]
    pub(crate) fn into_metadata(self) -> FileMetadata {
        self.metadata
    }
}
