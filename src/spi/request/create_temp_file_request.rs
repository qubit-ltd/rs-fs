// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated temporary-file creation request.

use crate::temp::TempOptions;

/// A facade-created temporary-file request.
pub struct CreateTempFileRequest {
    /// Validated temporary-file creation options.
    options: TempOptions,
}

impl CreateTempFileRequest {
    /// Creates this request inside the facade boundary.
    ///
    /// # Parameters
    /// - `options`: Validated temporary-file creation options.
    ///
    /// # Returns
    /// A provider temporary-file request.
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn new(options: TempOptions) -> Self {
        Self { options }
    }

    /// Returns requested temporary-file options.
    ///
    /// # Returns
    /// The immutable temporary-file creation options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &TempOptions {
        &self.options
    }
}
