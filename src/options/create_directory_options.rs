// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Directory creation options.

use crate::{
    NonSensitiveMetadata,
    UserMetadata,
};

/// Options controlling directory or collection creation.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub struct CreateDirectoryOptions {
    /// Whether missing parent directories should be created.
    recursive: bool,
    /// Whether an existing directory should be accepted.
    exists_ok: bool,
    /// User-defined metadata with validated non-sensitive structural keys.
    user_metadata: NonSensitiveMetadata,
}

impl Default for CreateDirectoryOptions {
    /// Creates non-recursive options that reject an existing directory.
    #[inline]
    fn default() -> Self {
        Self {
            recursive: false,
            exists_ok: false,
            user_metadata: NonSensitiveMetadata::new(),
        }
    }
}

impl CreateDirectoryOptions {
    /// Returns whether missing parent directories should be created.
    #[inline(always)]
    #[must_use]
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Returns whether an existing directory should be accepted.
    #[inline(always)]
    #[must_use]
    pub const fn exists_ok(&self) -> bool {
        self.exists_ok
    }

    /// Returns validated user-defined metadata.
    #[inline(always)]
    #[must_use]
    pub const fn user_metadata(&self) -> &NonSensitiveMetadata {
        &self.user_metadata
    }

    /// Replaces recursive parent creation.
    #[inline]
    #[must_use]
    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Replaces acceptance of an existing directory.
    #[inline]
    #[must_use]
    pub const fn with_exists_ok(mut self, exists_ok: bool) -> Self {
        self.exists_ok = exists_ok;
        self
    }

    /// Replaces user-defined metadata that has already passed key validation.
    #[inline(always)]
    #[must_use]
    pub fn with_user_metadata(mut self, metadata: UserMetadata) -> Self {
        self.user_metadata = NonSensitiveMetadata::from(metadata);
        self
    }
}
