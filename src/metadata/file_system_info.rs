// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Immutable configured filesystem information.

use crate::{
    FileSystemId,
    FsResult,
    FsScheme,
    NonSensitiveMetadata,
    PathSemantics,
    UserMetadata,
};
use std::fmt::Display;

/// Construction-time local snapshot describing one filesystem object.
#[derive(Clone, Debug, PartialEq)]
pub struct FileSystemInfo {
    id: FileSystemId,
    provider_id: Box<str>,
    schemes: Vec<String>,
    path_semantics: PathSemantics,
    provider_metadata: NonSensitiveMetadata,
}

impl FileSystemInfo {
    /// Creates a filesystem information snapshot without scheme aliases.
    #[inline]
    #[must_use]
    pub fn new(
        id: FileSystemId,
        provider_id: impl Display,
        path_semantics: PathSemantics,
    ) -> Self {
        Self {
            id,
            provider_id: provider_id.to_string().into(),
            schemes: Vec::new(),
            path_semantics,
            provider_metadata: NonSensitiveMetadata::new(),
        }
    }

    /// Adds one validated supported URI scheme.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when `scheme` is not a valid URI scheme.
    pub fn with_scheme(mut self, scheme: &str) -> FsResult<Self> {
        let scheme = FsScheme::parse(scheme)?.as_str().to_owned();
        if !self.schemes.contains(&scheme) {
            self.schemes.push(scheme);
        }
        Ok(self)
    }

    /// Replaces the scrubbed provider metadata snapshot.
    ///
    /// `metadata` has already rejected credential-like keys. Providers must
    /// expose secrets only through an external credential boundary, never
    /// through this debug-visible local snapshot.
    pub fn with_provider_metadata(
        mut self,
        metadata: UserMetadata,
    ) -> Self {
        self.provider_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Returns the configured filesystem identity.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> &FileSystemId {
        &self.id
    }

    /// Returns the provider identity that created this filesystem.
    #[inline]
    #[must_use]
    pub const fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns supported URI schemes in provider-defined order.
    #[inline]
    #[must_use]
    pub fn schemes(&self) -> &[String] {
        &self.schemes
    }

    /// Returns the provider-local path semantics.
    #[inline]
    #[must_use]
    pub const fn path_semantics(&self) -> PathSemantics {
        self.path_semantics
    }

    /// Returns scrubbed provider-specific information.
    #[inline]
    #[must_use]
    pub const fn provider_metadata(&self) -> &NonSensitiveMetadata {
        &self.provider_metadata
    }
}
