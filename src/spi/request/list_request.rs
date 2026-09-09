// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated directory-listing request.

use crate::directory::ListScope;
use crate::spi::ResolvedListOptions;

/// Validated listing scope and immutable provider-facing options.
pub struct ListRequest<'a> {
    /// Caller-selected prefix or configured namespace.
    scope: &'a ListScope,
    /// Facade-resolved enumeration behavior.
    options: ResolvedListOptions,
}

impl<'a> ListRequest<'a> {
    /// Creates a request after facade preflight.
    pub(crate) const fn new(scope: &'a ListScope, options: ResolvedListOptions) -> Self {
        Self { scope, options }
    }

    /// Returns the exact caller scope without synthesizing an empty path.
    #[inline]
    #[must_use]
    pub const fn scope(&self) -> &'a ListScope {
        self.scope
    }

    /// Returns immutable resolved listing options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &ResolvedListOptions {
        &self.options
    }
}
