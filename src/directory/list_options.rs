// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory listing options.

use std::time::Duration;
use std::time::Instant;

use crate::directory::ListFilter;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::SymlinkPolicy;
use crate::path::PathSemantics;
use crate::path::RelativePath;

/// Options controlling directory or prefix listing.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Default)]
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
    filter: Option<ListFilter>,
    /// Maximum returned descendant depth relative to the list root.
    max_depth: Option<usize>,
    /// Maximum number of entries returned to the caller.
    max_entries: Option<usize>,
    /// Maximum elapsed duration from stream creation.
    deadline: Option<Duration>,
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
        self.filter = prefix.map(ListFilter::Subtree);
        self
    }

    /// Returns the optional lexical prefix.
    #[inline(always)]
    #[must_use]
    pub fn prefix(&self) -> Option<&str> {
        match self.filter.as_ref() {
            Some(ListFilter::Subtree(prefix)) => Some(prefix),
            _ => None,
        }
    }

    /// Replaces the explicit listing filter.
    #[must_use]
    pub fn with_filter(mut self, filter: Option<ListFilter>) -> Self {
        self.filter = filter;
        self
    }

    /// Returns the explicit listing filter.
    #[must_use]
    pub fn filter(&self) -> Option<&ListFilter> {
        self.filter.as_ref()
    }

    /// Returns defaults for a flat object-key listing.
    #[must_use]
    pub fn object_keys() -> Self {
        Self {
            recursive: true,
            filter: Some(ListFilter::LiteralPrefix(String::new())),
            ..Self::default()
        }
    }

    /// Returns a copy with the maximum descendant depth replaced.
    #[inline]
    #[must_use]
    pub const fn with_max_depth(mut self, max_depth: Option<usize>) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Returns the optional maximum descendant depth.
    #[inline(always)]
    #[must_use]
    pub const fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    /// Returns a copy with the maximum returned entry count replaced.
    #[inline]
    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: Option<usize>) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Returns the optional maximum returned entry count.
    #[inline(always)]
    #[must_use]
    pub const fn max_entries(&self) -> Option<usize> {
        self.max_entries
    }

    /// Returns a copy with the maximum elapsed duration replaced.
    #[inline]
    #[must_use]
    pub const fn with_deadline(mut self, deadline: Option<Duration>) -> Self {
        self.deadline = deadline;
        self
    }

    /// Returns the optional maximum elapsed duration from stream creation.
    #[inline(always)]
    #[must_use]
    pub const fn deadline(&self) -> Option<Duration> {
        self.deadline
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
        self.validate_common()?;
        if let Some(ListFilter::Subtree(prefix)) = self.filter.as_ref() {
            let parsed = RelativePath::parse(prefix).map_err(|_| {
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list subtree must be a canonical relative path",
                )
            })?;
            if parsed.as_str() != prefix {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list subtree must be a canonical relative path",
                ));
            }
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now().checked_add(deadline).is_none())
        {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "list deadline exceeds the platform monotonic-clock range",
            ));
        }
        Ok(())
    }

    /// Validates options against the filesystem path semantics.
    pub fn validate_for(&self, semantics: PathSemantics) -> FsResult<()> {
        self.validate_common()?;
        self.validate_filter(semantics)
    }
    fn validate_common(&self) -> FsResult<()> {
        if self.page_size == Some(0) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "list page size must be greater than zero",
            ));
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now().checked_add(deadline).is_none())
        {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "list deadline exceeds the platform monotonic-clock range",
            ));
        }
        Ok(())
    }
    fn validate_filter(&self, semantics: PathSemantics) -> FsResult<()> {
        match (semantics, self.filter.as_ref()) {
            (PathSemantics::Hierarchical, Some(ListFilter::Subtree(prefix))) => {
                let parsed = RelativePath::parse(prefix).map_err(|_| {
                    FsError::new(
                        FsErrorKind::InvalidOptions,
                        FsOperation::List,
                        "list subtree must be a canonical relative path",
                    )
                })?;
                if parsed.as_str() != prefix {
                    return Err(FsError::new(
                        FsErrorKind::InvalidOptions,
                        FsOperation::List,
                        "list subtree must be a canonical relative path",
                    ));
                }
            }
            (PathSemantics::Hierarchical, Some(ListFilter::LiteralPrefix(_))) => {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "literal prefix requires flat path semantics",
                ));
            }
            (PathSemantics::ObjectKey | PathSemantics::ProviderSpecific, Some(ListFilter::Subtree(_))) => {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "subtree filter requires hierarchical path semantics",
                ));
            }
            (PathSemantics::ObjectKey | PathSemantics::ProviderSpecific, Some(ListFilter::LiteralPrefix(prefix)))
                if prefix.contains('\0') =>
            {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "literal prefix contains NUL",
                ));
            }
            _ => {}
        }
        if matches!(semantics, PathSemantics::ObjectKey | PathSemantics::ProviderSpecific)
            && (!self.recursive || self.max_depth.is_some())
        {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "flat listing requires recursive traversal without max_depth",
            ));
        }
        Ok(())
    }
}
