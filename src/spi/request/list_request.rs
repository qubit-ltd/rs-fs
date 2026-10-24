// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Validated directory-listing request.

use crate::directory::ListScope;
use crate::spi::ResolvedListOptions;

/// Validated listing scope and immutable provider-facing options.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::ListRequest;
///
/// fn inspect_list_request(request: ListRequest<'_>) {
///     let _scope = request.scope();
///     let _options = request.options();
/// }
///
/// // The facade supplies this request to a provider's `FileSystemSpi::list`.
/// let _ = inspect_list_request;
/// ```
pub struct ListRequest<'a> {
    /// Caller-selected prefix or configured namespace.
    scope: &'a ListScope,
    /// Facade-resolved enumeration behavior.
    options: ResolvedListOptions,
}

impl<'a> ListRequest<'a> {
    /// Creates a request after facade preflight.
    ///
    /// # Parameters
    /// - `scope`: Caller-selected listing scope validated by the facade.
    /// - `options`: Listing behavior resolved by the facade for the provider.
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
