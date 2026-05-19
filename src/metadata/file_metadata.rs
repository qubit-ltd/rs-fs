/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! File metadata model.

use std::time::SystemTime;

use qubit_metadata::Metadata;

use crate::{
    Checksum,
    FileType,
};

/// Stable and extensible metadata for one filesystem resource.
#[derive(Clone, Debug, PartialEq)]
pub struct FileMetadata {
    /// Provider-neutral resource type.
    pub file_type: FileType,
    /// Byte length when known.
    pub len: Option<u64>,
    /// Last modification time when known.
    pub modified_at: Option<SystemTime>,
    /// Creation time when known.
    pub created_at: Option<SystemTime>,
    /// Last access time when known.
    pub accessed_at: Option<SystemTime>,
    /// Provider version or HTTP-style ETag when known.
    pub etag: Option<String>,
    /// Content type when known.
    pub content_type: Option<String>,
    /// Content checksum when known.
    pub checksum: Option<Checksum>,
    /// User-defined metadata.
    pub user_metadata: Metadata,
    /// Provider-native metadata.
    pub provider_metadata: Metadata,
}

impl FileMetadata {
    /// Creates metadata with only a file type.
    ///
    /// # Parameters
    /// - `file_type`: Provider-neutral resource type.
    ///
    /// # Returns
    /// Metadata value with unknown optional fields.
    #[inline]
    #[must_use]
    pub fn new(file_type: FileType) -> Self {
        Self {
            file_type,
            len: None,
            modified_at: None,
            created_at: None,
            accessed_at: None,
            etag: None,
            content_type: None,
            checksum: None,
            user_metadata: Metadata::new(),
            provider_metadata: Metadata::new(),
        }
    }

    /// Tells whether this metadata describes a directory-like resource.
    ///
    /// # Returns
    /// `true` for directories and prefixes.
    #[inline]
    #[must_use]
    pub fn is_directory_like(&self) -> bool {
        matches!(self.file_type, FileType::Directory | FileType::Prefix)
    }
}
