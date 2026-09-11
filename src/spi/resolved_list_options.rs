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

use crate::directory::ListOptions;
use crate::metadata::SymlinkPolicy;

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
    pub(crate) const fn new(options: ListOptions, symlink_policy: SymlinkPolicy) -> Self {
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

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::ResolvedListOptions;
    use crate::directory::ListOptions;
    use crate::metadata::SymlinkPolicy;

    #[test]
    fn resolved_options_expose_their_values() {
        let options = ListOptions::default().with_recursive(true);
        let resolved = ResolvedListOptions::new(options.clone(), SymlinkPolicy::FollowWithinFileSystem);
        let options_accessor: fn(&ResolvedListOptions) -> &ListOptions = black_box(ResolvedListOptions::options);
        let policy_accessor: fn(&ResolvedListOptions) -> SymlinkPolicy = black_box(ResolvedListOptions::symlink_policy);

        assert_eq!(&options, options_accessor(&resolved));
        assert_eq!(SymlinkPolicy::FollowWithinFileSystem, policy_accessor(&resolved));
    }
}
