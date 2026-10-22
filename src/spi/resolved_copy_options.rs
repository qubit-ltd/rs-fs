// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Facade-resolved copy options.

use crate::copy::CopyOptions;
use crate::metadata::SymlinkPolicy;

/// Immutable options resolved by the facade before provider dispatch.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::ResolvedCopyOptions;
///
/// fn inspect_copy_options(options: &ResolvedCopyOptions) {
///     let _caller_options = options.options();
///     let _symlink_policy = options.symlink_policy();
/// }
///
/// // A provider receives this facade-created value in its copy request.
/// let _ = inspect_copy_options;
/// ```
#[derive(Clone)]
pub struct ResolvedCopyOptions {
    /// Caller options retained after facade validation and normalization.
    options: CopyOptions,
    /// Effective symbolic-link policy after applying the caller override.
    symlink_policy: SymlinkPolicy,
}

impl ResolvedCopyOptions {
    /// Creates this value inside the facade boundary.
    ///
    /// # Parameters
    /// - `options`: Validated caller options after normalization.
    /// - `symlink_policy`: Effective provider policy for this request.
    ///
    /// # Returns
    /// The resolved options exposed to the provider.
    #[inline]
    pub(crate) const fn new(options: CopyOptions, symlink_policy: SymlinkPolicy) -> Self {
        Self {
            options,
            symlink_policy,
        }
    }

    /// Returns the resolved options.
    ///
    /// # Returns
    /// The validated caller options retained by the facade.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &CopyOptions {
        &self.options
    }

    /// Returns the effective symbolic-link policy.
    ///
    /// # Returns
    /// The provider policy selected for this request.
    #[inline]
    #[must_use = "the resolved symbolic-link policy must be used"]
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }
}
