// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated temporary-directory creation request.

use crate::temp::TempOptions;

/// A facade-created temporary-directory request.
///
/// The facade supplies this value to the provider's directory-creation entry
/// point for inspection.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::CreateTempDirectoryRequest;
///
/// fn inspect_request(request: &CreateTempDirectoryRequest) {
///     let options = request.options();
///     let _ = options;
/// }
///
/// let _ = inspect_request;
/// ```
#[must_use = "pass the request to the provider or drop it explicitly"]
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
