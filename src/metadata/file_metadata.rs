// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File metadata model.

use std::time::SystemTime;

use crate::Checksum;
use crate::FileKind;
use crate::NonSensitiveMetadata;
use crate::ResourceVersion;
use crate::UserMetadata;

/// Stable and extensible metadata for one filesystem resource.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct FileMetadata {
    /// Provider-neutral resource kind.
    kind: FileKind,
    /// Byte length when known.
    len: Option<u64>,
    /// Last modification time when known.
    modified_at: Option<SystemTime>,
    /// Creation time when known.
    created_at: Option<SystemTime>,
    /// Last access time when known.
    accessed_at: Option<SystemTime>,
    /// Provider version or HTTP-style ETag when known.
    etag: Option<ResourceVersion>,
    /// Content type when known.
    content_type: Option<String>,
    /// Content checksum when known.
    checksum: Option<Checksum>,
    /// User-defined metadata with validated non-sensitive structural keys.
    user_metadata: NonSensitiveMetadata,
    /// Provider-native metadata with validated non-sensitive structural keys.
    provider_metadata: NonSensitiveMetadata,
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

    /// Returns the provider-neutral resource kind.
    #[inline]
    #[must_use]
    pub const fn kind(&self) -> &FileKind {
        &self.kind
    }

    /// Returns the known byte length, if any.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> Option<u64> {
        self.len
    }

    /// Returns whether the known byte length is zero.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        matches!(self.len, Some(0))
    }

    /// Returns the last modification time, if known.
    #[inline]
    #[must_use]
    pub const fn modified_at(&self) -> Option<SystemTime> {
        self.modified_at
    }

    /// Returns the creation time, if known.
    #[inline]
    #[must_use]
    pub const fn created_at(&self) -> Option<SystemTime> {
        self.created_at
    }

    /// Returns the last access time, if known.
    #[inline]
    #[must_use]
    pub const fn accessed_at(&self) -> Option<SystemTime> {
        self.accessed_at
    }

    /// Returns the provider version or ETag, if known.
    #[inline]
    #[must_use]
    pub const fn etag(&self) -> Option<&ResourceVersion> {
        self.etag.as_ref()
    }

    /// Returns the content type, if known.
    #[inline]
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Returns the content checksum, if known.
    #[inline]
    #[must_use]
    pub const fn checksum(&self) -> Option<&Checksum> {
        self.checksum.as_ref()
    }

    /// Returns validated user-defined metadata.
    #[inline]
    #[must_use]
    pub const fn user_metadata(&self) -> &NonSensitiveMetadata {
        &self.user_metadata
    }

    /// Returns validated provider-native metadata.
    #[inline]
    #[must_use]
    pub const fn provider_metadata(&self) -> &NonSensitiveMetadata {
        &self.provider_metadata
    }

    /// Replaces the resource kind.
    #[inline]
    #[must_use]
    pub fn with_kind(mut self, kind: FileKind) -> Self {
        self.kind = kind;
        self
    }

    /// Replaces the known byte length.
    #[inline]
    #[must_use]
    pub fn with_len(mut self, len: Option<u64>) -> Self {
        self.len = len;
        self
    }

    /// Replaces the last modification time.
    #[inline]
    #[must_use]
    pub fn with_modified_at(mut self, value: Option<SystemTime>) -> Self {
        self.modified_at = value;
        self
    }

    /// Replaces the creation time.
    #[inline]
    #[must_use]
    pub fn with_created_at(mut self, value: Option<SystemTime>) -> Self {
        self.created_at = value;
        self
    }

    /// Replaces the last access time.
    #[inline]
    #[must_use]
    pub fn with_accessed_at(mut self, value: Option<SystemTime>) -> Self {
        self.accessed_at = value;
        self
    }

    /// Replaces the provider version or ETag.
    #[inline]
    #[must_use]
    pub fn with_etag(mut self, value: Option<ResourceVersion>) -> Self {
        self.etag = value;
        self
    }

    /// Replaces the content type.
    #[inline]
    #[must_use]
    pub fn with_content_type(mut self, value: Option<String>) -> Self {
        self.content_type = value;
        self
    }

    /// Replaces the content checksum.
    #[inline]
    #[must_use]
    pub fn with_checksum(mut self, value: Option<Checksum>) -> Self {
        self.checksum = value;
        self
    }

    /// Replaces user-defined metadata that has already passed key validation.
    #[inline]
    #[must_use]
    pub fn with_user_metadata(mut self, metadata: UserMetadata) -> Self {
        self.user_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Replaces provider-native metadata that has already passed key
    /// validation.
    #[inline]
    #[must_use]
    pub fn with_provider_metadata(mut self, metadata: UserMetadata) -> Self {
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

    /// Tells whether this metadata describes a file-like resource.
    ///
    /// # Returns
    /// `true` for regular files and object-store objects.
    #[inline]
    #[must_use]
    pub fn is_file_like(&self) -> bool {
        matches!(self.kind, FileKind::File | FileKind::Object)
    }
}
