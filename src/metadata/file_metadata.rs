// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File metadata model.

use std::time::SystemTime;

use crate::{
    Checksum,
    FileKind,
    NonSensitiveMetadata,
    UserMetadata,
};

/// Stable and extensible metadata for one filesystem resource.
#[derive(Clone, Debug, PartialEq)]
pub struct FileMetadata {
    /// Provider-neutral resource kind.
    pub kind: FileKind,
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
    /// User-defined metadata with validated non-sensitive structural keys.
    pub user_metadata: NonSensitiveMetadata,
    /// Provider-native metadata with validated non-sensitive structural keys.
    pub provider_metadata: NonSensitiveMetadata,
}

impl FileMetadata {
    /// Creates metadata with only a file kind.
    ///
    /// # Parameters
    /// - `kind`: Provider-neutral resource kind.
    ///
    /// # Returns
    /// Metadata value with unknown optional fields.
    #[inline]
    #[must_use]
    pub fn new(kind: FileKind) -> Self {
        Self {
            kind,
            len: None,
            modified_at: None,
            created_at: None,
            accessed_at: None,
            etag: None,
            content_type: None,
            checksum: None,
            user_metadata: NonSensitiveMetadata::new(),
            provider_metadata: NonSensitiveMetadata::new(),
        }
    }

    /// Replaces user-defined metadata that has already passed key validation.
    #[inline]
    pub fn with_user_metadata(
        mut self,
        metadata: UserMetadata,
    ) -> Self {
        self.user_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Replaces provider-native metadata that has already passed key validation.
    #[inline]
    pub fn with_provider_metadata(
        mut self,
        metadata: UserMetadata,
    ) -> Self {
        self.provider_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Tells whether this metadata describes a directory-like resource.
    ///
    /// # Returns
    /// `true` for directories and prefixes.
    #[inline]
    #[must_use]
    pub fn is_directory_like(&self) -> bool {
        matches!(self.kind, FileKind::Directory | FileKind::Prefix)
    }
}
