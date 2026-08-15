// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory listing options.

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::SymlinkPolicy;
use crate::path::RelativePath;

/// Options controlling directory or prefix listing.
#[non_exhaustive]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListOptions {
    /// Whether listing should recurse into child containers.
    recursive: bool,
    /// Optional symbolic-link policy overriding the filesystem default.
    symlink_policy: Option<SymlinkPolicy>,
    /// Whether entries should include metadata when available.
    include_metadata: bool,
    /// Optional provider page size hint.
    page_size: Option<usize>,
    /// Optional lexical prefix filter relative to the requested list root.
    ///
    /// The filter uses canonical `/`-separated relative paths. For example,
    /// listing `/root` with `prefix: Some("nested/item")` matches
    /// `/root/nested/item`, while `prefix: Some("item")` only matches an
    /// immediate child named `item`.
    prefix: Option<String>,
}

impl ListOptions {
    /// Returns a copy with recursive traversal replaced.
    #[inline]
    #[must_use]
    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Returns whether traversal recurses into child containers.
    #[inline(always)]
    #[must_use]
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Returns a copy with the symbolic-link policy override replaced.
    #[inline]
    #[must_use]
    pub const fn with_symlink_policy(mut self, policy: SymlinkPolicy) -> Self {
        self.symlink_policy = Some(policy);
        self
    }

    /// Returns the optional symbolic-link policy override.
    #[inline(always)]
    #[must_use]
    pub const fn symlink_policy_override(&self) -> Option<SymlinkPolicy> {
        self.symlink_policy
    }

    /// Returns a copy with metadata inclusion replaced.
    #[inline]
    #[must_use]
    pub const fn with_include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Returns whether metadata is requested for entries.
    #[inline(always)]
    #[must_use]
    pub const fn include_metadata(&self) -> bool {
        self.include_metadata
    }

    /// Returns a copy with the page-size hint replaced.
    #[inline]
    #[must_use]
    pub const fn with_page_size(mut self, page_size: Option<usize>) -> Self {
        self.page_size = page_size;
        self
    }

    /// Returns the optional page-size hint.
    #[inline(always)]
    #[must_use]
    pub const fn page_size(&self) -> Option<usize> {
        self.page_size
    }

    /// Returns a copy with the lexical prefix replaced.
    #[inline]
    #[must_use]
    pub fn with_prefix(mut self, prefix: Option<String>) -> Self {
        self.prefix = prefix;
        self
    }

    /// Returns the optional lexical prefix.
    #[inline(always)]
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        self.prefix.as_deref()
    }

    /// Validates pagination and canonical provider-facing prefix values.
    ///
    /// # Errors
    ///
    /// Returns an invalid-options error when the page size is zero or the
    /// prefix is not a canonical relative path.
    pub fn validate(&self) -> FsResult<()> {
        if self.page_size == Some(0) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "list page size must be greater than zero",
            ));
        }
        if let Some(prefix) = self.prefix.as_deref() {
            let parsed = RelativePath::parse(prefix).map_err(|_| {
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list prefix must be a canonical relative path",
                )
            })?;
            if parsed.as_str() != prefix {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list prefix must be a canonical relative path",
                ));
            }
        }
        Ok(())
    }
}
