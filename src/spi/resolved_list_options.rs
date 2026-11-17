// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Facade-resolved listing options.

use crate::directory::ListOptions;
use crate::metadata::SymlinkPolicy;

/// Immutable options resolved by the facade before provider dispatch.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::directory::ListOptions;
/// use qubit_fs::metadata::SymlinkPolicy;
/// use qubit_fs::spi::ResolvedListOptions;
///
/// fn inspect_list_options(
///     options: &ResolvedListOptions,
/// ) -> (&ListOptions, SymlinkPolicy) {
///     (options.options(), options.symlink_policy())
/// }
///
/// // A provider calls this helper with the facade-created value from its list request.
/// ```
#[must_use = "pass the resolved listing options with the list request"]
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
    ///
    /// # Returns
    /// A snapshot containing validated listing options and the effective
    /// symlink policy.
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
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }
}
