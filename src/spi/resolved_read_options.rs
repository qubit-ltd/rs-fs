// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Facade-resolved read options.

use crate::read::ReadOptions;
use crate::spi::PrefixReadHint;

/// Immutable read options resolved by the facade before provider dispatch.
///
/// Providers receive this value after the facade has validated the caller's
/// options and selected any applicable prefix-read hint.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::ResolvedReadOptions;
///
/// fn inspect_read_options(options: &ResolvedReadOptions) {
///     let _caller_options = options.options();
///     let _prefix_hint = options.prefix_hint();
/// }
///
/// // A provider receives this facade-created value in its read request.
/// let _ = inspect_read_options;
/// ```
#[derive(Clone)]
pub struct ResolvedReadOptions {
    /// Caller options retained after facade validation and normalization.
    options: ReadOptions,
    /// Optional bound for the prefix a provider may read during one open.
    prefix_hint: Option<PrefixReadHint>,
}

impl ResolvedReadOptions {
    /// Creates resolved options without a provider optimization hint.
    ///
    /// # Parameters
    /// - `options`: Validated caller options retained for provider dispatch.
    ///
    /// # Returns
    /// Resolved options without a prefix-read hint.
    #[inline]
    #[must_use]
    pub(crate) const fn new(options: ReadOptions) -> Self {
        Self {
            options,
            prefix_hint: None,
        }
    }

    /// Adds a non-binding bounded-prefix hint.
    ///
    /// # Parameters
    /// - `hint`: The maximum prefix size the provider may use as a hint.
    ///
    /// # Returns
    /// Resolved options carrying the supplied hint.
    #[inline]
    #[must_use]
    pub(crate) const fn with_prefix_hint(mut self, hint: PrefixReadHint) -> Self {
        self.prefix_hint = Some(hint);
        self
    }

    /// Returns the validated read options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ReadOptions {
        &self.options
    }

    /// Returns the optional prefix hint.
    #[inline]
    #[must_use]
    pub const fn prefix_hint(&self) -> Option<PrefixReadHint> {
        self.prefix_hint
    }
}
