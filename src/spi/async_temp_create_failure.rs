// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Recoverable failure returned by asynchronous temporary-resource creation.

use crate::error::FsError;

use super::AsyncTempResourceSpi;

/// Asynchronous creation error that can retain cleanup authority for a
/// partially created temporary resource.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsError;
/// use qubit_fs::error::{FsErrorKind, FsOperation};
/// use qubit_fs::spi::AsyncTempCreateFailure;
///
/// let failure = AsyncTempCreateFailure::new(
///     FsError::new(FsErrorKind::Io, FsOperation::CreateTemp, "create failed"),
///     None,
/// );
/// let (error, recovery) = failure.into_parts();
/// assert_eq!(FsErrorKind::Io, error.kind());
/// assert!(recovery.is_none());
/// ```
#[must_use = "retain cleanup authority when asynchronous temporary creation may have changed state"]
pub struct AsyncTempCreateFailure {
    error: FsError,
    session: Option<Box<dyn AsyncTempResourceSpi>>,
}

impl AsyncTempCreateFailure {
    /// Creates a failure with an optional provider-owned cleanup session.
    ///
    /// # Parameters
    /// - `error`: The original temporary-creation error.
    /// - `session`: Cleanup authority for any partially created resource.
    ///
    /// # Returns
    /// A failure retaining the error and optional cleanup session.
    pub fn new(error: FsError, session: Option<Box<dyn AsyncTempResourceSpi>>) -> Self {
        Self { error, session }
    }

    /// Transfers the original error and any cleanup session.
    ///
    /// # Returns
    /// The original error and optional provider cleanup session.
    pub fn into_parts(self) -> (FsError, Option<Box<dyn AsyncTempResourceSpi>>) {
        (self.error, self.session)
    }
}

impl std::fmt::Debug for AsyncTempCreateFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AsyncTempCreateFailure")
            .field("error", &self.error)
            .field("has_session", &self.session.is_some())
            .finish()
    }
}
