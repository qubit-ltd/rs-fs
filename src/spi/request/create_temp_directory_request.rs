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

use crate::TempDirectoryOptions;

/// A facade-created temporary-directory request.
pub struct CreateTempDirectoryRequest {
    /// Validated temporary-directory creation options.
    options: TempDirectoryOptions,
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
    pub(crate) const fn new(options: TempDirectoryOptions) -> Self {
        Self { options }
    }

    /// Returns requested temporary-directory options.
    ///
    /// # Returns
    /// The immutable temporary-directory creation options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &TempDirectoryOptions {
        &self.options
    }
}
