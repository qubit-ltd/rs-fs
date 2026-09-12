// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Facade-resolved read options.

use crate::read::ReadOptions;
use crate::spi::PrefixReadHint;

/// Immutable reader options resolved by the facade.
#[derive(Clone)]
pub struct ResolvedReadOptions {
    options: ReadOptions,
    prefix_hint: Option<PrefixReadHint>,
}

impl ResolvedReadOptions {
    /// Creates resolved options without a provider optimization hint.
    #[inline]
    pub(crate) const fn new(options: ReadOptions) -> Self {
        Self {
            options,
            prefix_hint: None,
        }
    }

    /// Adds a non-binding bounded-prefix hint.
    #[inline]
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
