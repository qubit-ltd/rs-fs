// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade tests.
//! Directory creation outcome.

/// Result returned after a directory creation request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::directory::CreateDirectoryOutcome;
///
/// let outcome = CreateDirectoryOutcome::new(false);
/// assert!(!outcome.already_existed());
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateDirectoryOutcome {
    /// Whether an existing directory satisfied the request.
    already_existed: bool,
    /// Number of ancestor directories created when the provider reports it.
    created_ancestors: Option<u64>,
}

impl CreateDirectoryOutcome {
    /// Creates an outcome. `already_existed` reports an accepted existing
    /// directory.
    ///
    /// # Parameters
    ///
    /// * `already_existed` indicates whether an existing directory satisfied
    ///   the request.
    ///
    /// # Returns
    ///
    /// A new outcome without a provider-reported ancestor count.
    #[inline]
    #[must_use]
    pub const fn new(already_existed: bool) -> Self {
        Self {
            already_existed,
            created_ancestors: None,
        }
    }

    /// Returns whether an existing directory satisfied the request.
    ///
    /// # Returns
    ///
    /// `true` when an existing directory satisfied the request.
    #[inline]
    #[must_use]
    pub const fn already_existed(self) -> bool {
        self.already_existed
    }

    /// Attaches the number of ancestor directories created, when known.
    ///
    /// # Parameters
    ///
    /// * `count` is the number of ancestor directories created.
    ///
    /// # Returns
    ///
    /// The updated outcome with the reported ancestor count.
    #[inline]
    #[must_use]
    pub const fn with_created_ancestors(mut self, count: u64) -> Self {
        self.created_ancestors = Some(count);
        self
    }

    /// Returns the number of created ancestor directories, when reported.
    ///
    /// # Returns
    ///
    /// `Some(count)` when the provider reported the number of created
    /// ancestors, or `None` when it did not.
    #[inline]
    pub const fn created_ancestors(self) -> Option<u64> {
        self.created_ancestors
    }
}
