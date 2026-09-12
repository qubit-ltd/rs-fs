// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Facade-resolved copy options.

use crate::copy::CopyOptions;
use crate::metadata::SymlinkPolicy;

/// Immutable options resolved by the facade before provider dispatch.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::CopyOptions;
/// use qubit_fs::metadata::SymlinkPolicy;
/// use qubit_fs::spi::ResolvedCopyOptions;
///
/// let options = CopyOptions::default();
/// assert!(std::any::type_name::<ResolvedCopyOptions>().contains("ResolvedCopyOptions"));
/// assert_eq!(SymlinkPolicy::Reject, SymlinkPolicy::Reject);
/// let _ = options;
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
    #[inline]
    pub(crate) const fn new(options: CopyOptions, symlink_policy: SymlinkPolicy) -> Self {
        Self {
            options,
            symlink_policy,
        }
    }

    /// Returns the resolved options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &CopyOptions {
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

    use super::ResolvedCopyOptions;
    use crate::copy::CopyOptions;
    use crate::metadata::SymlinkPolicy;

    #[test]
    fn resolved_options_expose_their_values() {
        let options = CopyOptions::default();
        let resolved = ResolvedCopyOptions::new(options.clone(), SymlinkPolicy::FollowWithinFileSystem);
        let options_accessor: fn(&ResolvedCopyOptions) -> &CopyOptions = black_box(ResolvedCopyOptions::options);
        let policy_accessor: fn(&ResolvedCopyOptions) -> SymlinkPolicy = black_box(ResolvedCopyOptions::symlink_policy);

        assert_eq!(&options, options_accessor(&resolved));
        assert_eq!(SymlinkPolicy::FollowWithinFileSystem, policy_accessor(&resolved));
    }
}
