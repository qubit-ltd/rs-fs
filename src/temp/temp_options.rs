// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary resource creation options.

use crate::path::Path;

/// Options shared by temporary file and temporary directory creation.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TempOptions {
    /// Parent directory or prefix for the temporary resource.
    parent: Option<Path>,
    /// Generated resource name prefix.
    prefix: String,
    /// Generated resource name suffix.
    suffix: String,
    /// Whether a missing parent directory is created.
    create_parent: bool,
}

impl TempOptions {
    /// Creates empty temporary-resource options without parent creation.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            parent: None,
            prefix: String::new(),
            suffix: String::new(),
            create_parent: false,
        }
    }

    /// Returns the optional parent directory or prefix.
    #[inline(always)]
    #[must_use]
    pub const fn parent(&self) -> Option<&Path> {
        self.parent.as_ref()
    }

    /// Returns the generated resource name prefix.
    #[inline(always)]
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the generated resource name suffix.
    #[inline(always)]
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Returns whether missing parent directories are created.
    #[inline(always)]
    #[must_use]
    pub const fn creates_parent(&self) -> bool {
        self.create_parent
    }

    /// Replaces the optional parent directory or prefix.
    #[inline]
    #[must_use]
    pub fn with_parent(mut self, parent: Option<Path>) -> Self {
        self.parent = parent;
        self
    }

    /// Replaces the generated resource name prefix.
    #[inline]
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Replaces the generated resource name suffix.
    #[inline]
    #[must_use]
    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Replaces whether missing parent directories are created.
    #[inline]
    #[must_use]
    pub const fn with_create_parent(mut self, create: bool) -> Self {
        self.create_parent = create;
        self
    }
}

impl Default for TempOptions {
    /// Creates empty temporary-resource options without parent creation.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
