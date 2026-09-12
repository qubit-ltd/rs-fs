// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated temporary-directory creation request.

use crate::temp::TempOptions;

/// A facade-created temporary-directory request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::CreateTempDirectoryRequest;
/// use qubit_fs::temp::TempOptions;
///
/// assert!(std::any::type_name::<CreateTempDirectoryRequest>().contains("CreateTempDirectoryRequest"));
/// assert_eq!(TempOptions::default(), TempOptions::new());
/// ```
pub struct CreateTempDirectoryRequest {
    /// Validated temporary-directory creation options.
    options: TempOptions,
}

impl CreateTempDirectoryRequest {
    /// Creates this request inside the facade boundary.
    ///
    /// # Parameters
    /// - `options`: Validated temporary-directory creation options.
    ///
    /// # Returns
    /// A provider temporary-directory request.
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn new(options: TempOptions) -> Self {
        Self { options }
    }

    /// Returns requested temporary-directory options.
    ///
    /// # Returns
    /// The immutable temporary-directory creation options.
    #[inline]
    #[must_use]
    pub const fn options(&self) -> &TempOptions {
        &self.options
    }
}
