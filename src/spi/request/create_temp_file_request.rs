// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Validated temporary-file creation request.

use crate::temp::TempOptions;

/// A facade-created temporary-file request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::CreateTempFileRequest;
/// use qubit_fs::temp::TempOptions;
///
/// fn inspect_temp_file_request(request: &CreateTempFileRequest) -> &TempOptions {
///     request.options()
/// }
///
/// let _ = inspect_temp_file_request;
/// ```
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
    /// A request containing the validated temporary-file options.
    #[allow(dead_code)]
    #[must_use]
    #[inline]
    pub(crate) const fn new(options: TempOptions) -> Self {
        Self { options }
    }

    /// Returns requested temporary-file options.
    ///
    /// # Returns
    /// The immutable temporary-file creation options.
    #[must_use]
    #[inline]
    pub const fn options(&self) -> &TempOptions {
        &self.options
    }
}
