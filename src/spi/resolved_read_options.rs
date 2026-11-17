// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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
/// use qubit_fs::read::ReadOptions;
/// use qubit_fs::spi::PrefixReadHint;
/// use qubit_fs::spi::ResolvedReadOptions;
///
/// fn inspect_read_options(
///     options: &ResolvedReadOptions,
/// ) -> (&ReadOptions, Option<PrefixReadHint>) {
///     (options.options(), options.prefix_hint())
/// }
///
/// // A provider calls this helper with the facade-created value from its read request.
/// ```
#[must_use = "pass the resolved read options with the read request"]
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
    pub(crate) const fn new(options: ReadOptions) -> Self {
        Self {
            options,
            prefix_hint: None,
        }
    }

    /// Returns the validated read options.
    ///
    /// # Returns
    /// The caller's read options retained after facade validation and
    /// normalization.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ReadOptions {
        &self.options
    }

    /// Returns the optional prefix hint.
    ///
    /// # Returns
    /// `Some(hint)` when the facade selected a prefix-read hint, or `None` when
    /// no hint applies to this request.
    #[inline]
    pub const fn prefix_hint(&self) -> Option<PrefixReadHint> {
        self.prefix_hint
    }

    /// Adds a non-binding bounded-prefix hint.
    ///
    /// # Parameters
    /// - `hint`: The maximum prefix size the provider may use as a hint.
    ///
    /// # Returns
    /// Resolved options carrying the supplied hint.
    #[inline]
    pub(crate) const fn with_prefix_hint(mut self, hint: PrefixReadHint) -> Self {
        self.prefix_hint = Some(hint);
        self
    }
}
