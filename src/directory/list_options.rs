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
///
/// Options are validated by [`crate::FileSystem::list`] before a provider
/// session is opened. A page-size hint is bounded by the provider's declared
/// limit; `max_depth` and `max_entries` are caller-side safety bounds.
///
/// # Examples
///
/// ```
/// use qubit_fs::directory::{ListFilter, ListOptions};
///
/// let options = ListOptions::default()
///     .with_page_size(Some(100))
///     .with_max_depth(Some(2))
///     .with_max_entries(Some(500))
///     .with_filter(Some(ListFilter::Subtree("reports".to_owned())));
/// assert_eq!(options.page_size(), Some(100));
/// assert_eq!(options.max_depth(), Some(2));
/// assert_eq!(options.max_entries(), Some(500));
/// assert!(options.validate().is_ok());
/// ```
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ListOptions {
    /// Whether listing should recurse into child containers.
    ///
    /// Without a filter, `false` limits traversal to immediate children. A
    /// [`ListFilter::Subtree`] still selects that subtree and its descendants;
    /// `max_depth` controls how far below the list root results may extend.
    recursive: bool,
    /// Optional symbolic-link policy overriding the filesystem default.
    symlink_policy: Option<SymlinkPolicy>,
    /// Whether entries should include metadata when available.
    include_metadata: bool,
    /// Optional provider page size hint.
    page_size: Option<usize>,
    /// Optional lexical prefix filter relative to the requested list root.
    ///
    /// [`ListFilter::Subtree`] uses canonical `/`-separated relative paths.
    /// Listing `/root` with `Subtree("nested")` selects `/root/nested` and
    /// its descendants, including deeper entries when `recursive` is false.
    /// `max_depth` can limit the returned descendant depth relative to `/root`.
    filter: Option<ListFilter>,
    /// Maximum returned descendant depth relative to the list root.
    max_depth: Option<usize>,
    /// Maximum number of entries returned to the caller.
    max_entries: Option<usize>,
    /// Maximum elapsed duration from stream creation.
    deadline: Option<Duration>,
}

impl ListOptions {
    /// Returns defaults for a flat object-key listing.
    #[must_use]
    pub fn object_keys() -> Self {
        Self {
            recursive: true,
            filter: Some(ListFilter::LiteralPrefix(String::new())),
            ..Self::default()
        }
    }

    /// Returns whether traversal recurses into child containers.
    ///
    /// When a subtree filter is present, the filter still selects its
    /// descendants; `max_depth` bounds their depth. Without a filter,
    /// `false` selects immediate children only.
    #[inline]
    #[must_use]
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Returns the optional symbolic-link policy override.
    ///
    /// # Returns
    /// `Some` with the configured override, or `None` when the filesystem
    /// default policy applies.
    #[inline]
    pub const fn symlink_policy_override(&self) -> Option<SymlinkPolicy> {
        self.symlink_policy
    }

    /// Returns whether metadata is requested for entries.
    #[inline]
    #[must_use]
    pub const fn include_metadata(&self) -> bool {
        self.include_metadata
    }

    /// Returns the optional page-size hint.
    ///
    /// # Returns
    /// `Some` with the provider page-size hint, or `None` when no hint is set.
    #[inline]
    pub const fn page_size(&self) -> Option<usize> {
        self.page_size
    }

    /// Returns the optional subtree prefix set through the legacy prefix API.
    ///
    /// # Returns
    /// `Some` with the configured subtree prefix, or `None` when no subtree
    /// prefix filter is configured.
    #[inline]
    pub fn prefix(&self) -> Option<&str> {
        match self.filter.as_ref() {
            Some(ListFilter::Subtree(prefix)) => Some(prefix),
            _ => None,
        }
    }

    /// Returns the explicit listing filter.
    ///
    /// # Returns
    /// `Some` with the configured filter, or `None` when listing is unfiltered.
    pub fn filter(&self) -> Option<&ListFilter> {
        self.filter.as_ref()
    }

    /// Returns the optional maximum descendant depth.
    ///
    /// # Returns
    /// `Some` with the maximum descendant depth, or `None` when no depth limit
    /// is configured.
    #[inline]
    pub const fn max_depth(&self) -> Option<usize> {
        self.max_depth
    }

    /// Returns the optional maximum returned entry count.
    ///
    /// # Returns
    /// `Some` with the maximum entry count, or `None` when no count limit is
    /// configured.
    #[inline]
    pub const fn max_entries(&self) -> Option<usize> {
        self.max_entries
    }

    /// Returns the optional maximum elapsed duration from stream creation.
    ///
    /// # Returns
    /// `Some` with the elapsed-time limit, or `None` when no deadline is set.
    #[inline]
    pub const fn deadline(&self) -> Option<Duration> {
        self.deadline
    }

    /// Returns a copy with recursive traversal replaced.
    ///
    /// # Parameters
    /// - `recursive`: Whether traversal should recurse into child containers.
    #[inline]
    #[must_use]
    pub const fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Returns a copy with the symbolic-link policy override replaced.
    ///
    /// # Parameters
    /// - `policy`: Symbolic-link policy to use instead of the filesystem
    ///   default.
    #[inline]
    #[must_use]
    pub const fn with_symlink_policy(mut self, policy: SymlinkPolicy) -> Self {
        self.symlink_policy = Some(policy);
        self
    }

    /// Returns a copy with metadata inclusion replaced.
    ///
    /// # Parameters
    /// - `include`: Whether entries should include metadata when available.
    #[inline]
    #[must_use]
    pub const fn with_include_metadata(mut self, include: bool) -> Self {
        self.include_metadata = include;
        self
    }

    /// Returns a copy with the page-size hint replaced.
    ///
    /// # Parameters
    /// - `page_size`: Provider page-size hint, or `None` to leave it
    ///   unspecified.
    #[inline]
    #[must_use]
    pub const fn with_page_size(mut self, page_size: Option<usize>) -> Self {
        self.page_size = page_size;
        self
    }

    /// Returns a copy with the subtree prefix replaced through the legacy API.
    ///
    /// # Parameters
    /// - `prefix`: Relative subtree path to select, or `None` to clear the
    ///   filter.
    #[inline]
    #[must_use]
    pub fn with_prefix(mut self, prefix: Option<String>) -> Self {
        self.filter = prefix.map(ListFilter::Subtree);
        self
    }

    /// Replaces the explicit listing filter.
    ///
    /// # Parameters
    /// - `filter`: Filter to apply, or `None` to list without a filter.
    #[inline]
    #[must_use]
    pub fn with_filter(mut self, filter: Option<ListFilter>) -> Self {
        self.filter = filter;
        self
    }

    /// Returns a copy with the maximum descendant depth replaced.
    ///
    /// # Parameters
    /// - `max_depth`: Maximum descendant depth, or `None` for no depth limit.
    #[inline]
    #[must_use]
    pub const fn with_max_depth(mut self, max_depth: Option<usize>) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Returns a copy with the maximum returned entry count replaced.
    ///
    /// # Parameters
    /// - `max_entries`: Maximum returned entry count, or `None` for no count
    ///   limit.
    #[inline]
    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: Option<usize>) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Returns a copy with the maximum elapsed duration replaced.
    ///
    /// # Parameters
    /// - `deadline`: Maximum elapsed duration, or `None` for no deadline.
    #[inline]
    #[must_use]
    pub const fn with_deadline(mut self, deadline: Option<Duration>) -> Self {
        self.deadline = deadline;
        self
    }

    /// Validates pagination and canonical provider-facing prefix values.
    ///
    /// # Errors
    ///
    /// Returns `FsError` with `InvalidOptions` when the page size is zero, a
    /// subtree prefix is not canonical, or the deadline exceeds the platform
    /// monotonic-clock range.
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
        Ok(())
    }

    /// Validates options against the filesystem path semantics.
    ///
    /// # Parameters
    /// - `semantics`: Provider path semantics used to validate the filter.
    ///
    /// # Errors
    /// Returns `FsError` with `InvalidOptions` when the page size is zero, the
    /// deadline exceeds the platform monotonic-clock range, or the filter is
    /// invalid or incompatible with `semantics`.
    pub fn validate_for(&self, semantics: PathSemantics) -> FsResult<()> {
        self.validate_common()?;
        self.validate_filter(semantics)
    }

    /// Validates options that do not depend on path semantics.
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

    /// Validates the selected filter against provider path semantics.
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
