// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Recoverable failure returned by temporary-resource creation.

use crate::error::FsError;

use super::TempResourceSpi;

/// Creation error that can retain cleanup authority for a partially created
/// temporary resource.
#[must_use = "retain cleanup authority when temporary creation may have changed state"]
pub struct TempCreateFailure {
    error: FsError,
    session: Option<Box<dyn TempResourceSpi>>,
}

impl TempCreateFailure {
    /// Creates a failure with an optional provider-owned cleanup session.
    ///
    /// # Parameters
    /// - `error`: The original temporary-creation error.
    /// - `session`: Cleanup authority for any partially created resource.
    ///
    /// # Returns
    /// A failure retaining the error and optional cleanup session.
    pub fn new(error: FsError, session: Option<Box<dyn TempResourceSpi>>) -> Self {
        Self { error, session }
    }

    /// Transfers the original error and any cleanup session.
    ///
    /// # Returns
    /// The original error and optional provider cleanup session.
    pub fn into_parts(self) -> (FsError, Option<Box<dyn TempResourceSpi>>) {
        (self.error, self.session)
    }
}

impl std::fmt::Debug for TempCreateFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TempCreateFailure")
            .field("error", &self.error)
            .field("has_session", &self.session.is_some())
            .finish()
    }
}
