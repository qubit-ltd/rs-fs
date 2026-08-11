// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Facade-resolved listing options.

use crate::ListOptions;
use crate::SymlinkPolicy;

/// Immutable options resolved by the facade before provider dispatch.
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
    pub(crate) const fn new(
        options: ListOptions,
        symlink_policy: SymlinkPolicy,
    ) -> Self {
        Self {
            options,
            symlink_policy,
        }
    }

    /// Returns the resolved options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &ListOptions {
        &self.options
    }

    /// Returns the effective symbolic-link policy.
    #[inline(always)]
    #[must_use = "the resolved symbolic-link policy must be used"]
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }
}
