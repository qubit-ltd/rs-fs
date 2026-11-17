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
///
/// # Examples
///
/// ```rust
/// use qubit_fs::temp::TempOptions;
///
/// assert_eq!(TempOptions::default(), TempOptions::new());
/// ```
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
    ///
    /// # Returns
    /// Default options that let the provider select a location.
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

    /// Returns the requested temporary-resource location, if any.
    ///
    /// # Returns
    /// `Some(parent)` when a location was requested, otherwise `None`.
    #[inline]
    pub const fn parent(&self) -> Option<&Path> {
        self.parent.as_ref()
    }

    /// Returns the generated resource name prefix.
    ///
    /// # Returns
    /// The configured prefix, or an empty string when none was set.
    #[inline]
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// Returns the generated resource name suffix.
    ///
    /// # Returns
    /// The configured suffix, or an empty string when none was set.
    #[inline]
    #[must_use]
    pub fn suffix(&self) -> &str {
        &self.suffix
    }

    /// Returns whether missing parent directories are created.
    ///
    /// # Returns
    /// `true` when the provider should create missing parents.
    #[inline]
    #[must_use]
    pub const fn creates_parent(&self) -> bool {
        self.create_parent
    }

    /// Replaces the requested temporary-resource location.
    ///
    /// When set, the facade requires hierarchical temporary paths to be strict
    /// descendants of this path and object-key paths to retain its raw prefix.
    /// A provider may return an absolute logical path for a relative
    /// hierarchical parent when the exact component prefix is preserved.
    /// Provider-specific path semantics remain provider-defined. When `None`,
    /// the provider selects the location.
    ///
    /// # Parameters
    /// - `parent`: Requested temporary-resource parent, or `None` to let the
    ///   provider choose the location.
    ///
    /// # Returns
    /// These options with the requested parent location.
    #[inline]
    #[must_use]
    pub fn with_parent(mut self, parent: Option<Path>) -> Self {
        self.parent = parent;
        self
    }

    /// Replaces the generated resource name prefix.
    ///
    /// # Parameters
    /// - `prefix`: Name prefix converted into an owned string.
    ///
    /// # Returns
    /// These options with the generated-name prefix replaced.
    #[inline]
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Replaces the generated resource name suffix.
    ///
    /// # Parameters
    /// - `suffix`: Name suffix converted into an owned string.
    ///
    /// # Returns
    /// These options with the generated-name suffix replaced.
    #[inline]
    #[must_use]
    pub fn with_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.suffix = suffix.into();
        self
    }

    /// Replaces whether missing parent directories are created.
    ///
    /// # Parameters
    /// - `create`: Whether the provider should create a missing parent.
    ///
    /// # Returns
    /// These options with the missing-parent creation setting replaced.
    #[inline]
    #[must_use]
    pub const fn with_create_parent(mut self, create: bool) -> Self {
        self.create_parent = create;
        self
    }
}

impl Default for TempOptions {
    /// Creates empty temporary-resource options without parent creation.
    ///
    /// # Returns
    /// Default options that let the provider select a location.
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
