// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! File information captured as part of opening a stream.

use crate::{FileLocation, FileMetadata};

/// Stable file identity plus an optional metadata snapshot captured at open.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenedFileInfo {
    location: FileLocation,
    metadata: Option<FileMetadata>,
}

impl OpenedFileInfo {
    /// Creates opened-file information without an extra metadata lookup.
    ///
    /// # Parameters
    /// - `location`: File identity captured by the provider.
    ///
    /// # Returns
    /// Opened-file information with no metadata snapshot.
    #[inline]
    #[must_use]
    pub fn new(location: FileLocation) -> Self {
        Self {
            location,
            metadata: None,
        }
    }

    /// Attaches metadata already obtained while opening the file.
    ///
    /// Providers should not perform an extra remote `stat` only to populate
    /// this optional snapshot.
    ///
    /// # Parameters
    /// - `metadata`: Metadata observed during open.
    ///
    /// # Returns
    /// Updated opened-file information.
    #[inline]
    #[must_use]
    pub fn with_metadata(mut self, metadata: FileMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Returns the stable opened location.
    ///
    /// # Returns
    /// The location captured at open time.
    #[inline]
    #[must_use]
    pub fn location(&self) -> &FileLocation {
        &self.location
    }

    /// Returns the optional metadata snapshot captured during open.
    ///
    /// This is not live metadata. Call [`crate::FileSystem::stat`] when a
    /// current view is required.
    ///
    /// # Returns
    /// The optional open-time snapshot.
    #[inline]
    #[must_use]
    pub fn metadata(&self) -> Option<&FileMetadata> {
        self.metadata.as_ref()
    }

    /// Replaces the location when a registry-bound resource adds canonical
    /// identity that was unavailable to the provider-local open operation.
    #[inline]
    pub(crate) fn replace_location(&mut self, location: FileLocation) {
        self.location = location;
    }
}
