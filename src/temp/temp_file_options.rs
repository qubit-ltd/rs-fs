// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Temporary file creation options.

use crate::Path;

/// Options controlling temporary file creation.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TempFileOptions {
    /// Parent directory or prefix for the temporary file.
    parent: Option<Path>,
    /// Name prefix.
    prefix: String,
    /// Name suffix.
    suffix: String,
}

impl Default for TempFileOptions {
    #[inline]
    fn default() -> Self {
        Self {
            parent: None,
            prefix: ".tmp-".to_owned(),
            suffix: String::new(),
        }
    }
}

impl TempFileOptions {
    /// Returns the optional parent directory or prefix.
    #[inline(always)]
    #[must_use]
    pub const fn parent(&self) -> Option<&Path> {
        self.parent.as_ref()
    }

    /// Returns the generated file name prefix.
    #[inline(always)]
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the generated file name suffix.
    #[inline(always)]
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Replaces the optional parent directory or prefix.
    #[inline]
    #[must_use]
    pub fn with_parent(mut self, parent: Option<Path>) -> Self {
        self.parent = parent;
        self
    }

    /// Replaces the generated file name prefix.
    #[inline]
    #[must_use]
    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.prefix = prefix;
        self
    }

    /// Replaces the generated file name suffix.
    #[inline]
    #[must_use]
    pub fn with_suffix(mut self, suffix: String) -> Self {
        self.suffix = suffix;
        self
    }
}
