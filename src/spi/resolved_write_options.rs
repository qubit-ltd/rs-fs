// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Facade-resolved write options.

use crate::write::WriteOptions;

/// Immutable writer options resolved by the facade before provider dispatch.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::ResolvedWriteOptions;
///
/// fn inspect_write_options(options: &ResolvedWriteOptions) {
///     let _caller_options = options.options();
/// }
///
/// // A provider receives this facade-created value in its write request.
/// let _ = inspect_write_options;
/// ```
#[derive(Clone)]
pub struct ResolvedWriteOptions {
    /// Caller options retained after facade validation and normalization.
    options: WriteOptions,
}

impl ResolvedWriteOptions {
    /// Creates this value inside the facade boundary.
    ///
    /// # Parameters
    /// - `options`: Caller options after facade validation and normalization.
    ///
    /// # Returns
    /// An immutable provider-facing option envelope.
    #[inline]
    pub(crate) const fn new(options: WriteOptions) -> Self {
        Self { options }
    }

    /// Returns the resolved options.
    ///
    /// # Returns
    /// The validated options retained by this envelope.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &WriteOptions {
        &self.options
    }
}
