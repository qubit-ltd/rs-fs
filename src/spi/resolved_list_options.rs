// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Facade-resolved listing options.

use crate::directory::ListOptions;
use crate::metadata::SymlinkPolicy;

/// Immutable options resolved by the facade before provider dispatch.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::ResolvedListOptions;
///
/// fn inspect_list_options(options: &ResolvedListOptions) {
///     let _caller_options = options.options();
///     let _symlink_policy = options.symlink_policy();
/// }
///
/// // A provider receives this facade-created value in its list request.
/// let _ = inspect_list_options;
/// ```
#[derive(Clone)]
pub struct ResolvedListOptions {
    /// Caller options retained after facade validation and normalization.
    options: ListOptions,
    /// Effective symbolic-link policy after applying the caller override.
    symlink_policy: SymlinkPolicy,
}

impl ResolvedListOptions {
    /// Creates this value inside the facade boundary.
    ///
    /// # Parameters
    /// - `options`: Validated caller options after normalization.
    /// - `symlink_policy`: Effective provider policy for this request.
    #[inline]
    pub(crate) const fn new(options: ListOptions, symlink_policy: SymlinkPolicy) -> Self {
        Self {
            options,
            symlink_policy,
        }
    }

    /// Returns the caller's validated and normalized listing options.
    ///
    /// The returned value is borrowed from this facade-created snapshot, so a
    /// provider can inspect request settings without changing them.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ListOptions {
        &self.options
    }

    /// Returns the symbolic-link policy selected for this provider request.
    ///
    /// This value already incorporates the caller's override and the
    /// filesystem default, so providers should apply it directly.
    #[inline]
    #[must_use = "the resolved symbolic-link policy must be used"]
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }
}
