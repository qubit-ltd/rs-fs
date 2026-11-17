// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Failure to observe a final effect fact.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectObservationError;
///
/// let error = FsEffectObservationError::Poisoned;
/// assert_eq!(error, FsEffectObservationError::Poisoned);
/// ```
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsEffectObservationError {
    /// The shared fact lock was poisoned by a panicking thread.
    Poisoned,
}
impl Display for FsEffectObservationError {
    /// Writes the stable diagnostic for a poisoned observer lock.
    ///
    /// # Parameters
    ///
    /// - `f`: Formatter receiving the diagnostic text.
    ///
    /// # Returns
    /// The formatter result, including any formatting error.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("filesystem effect fact lock is poisoned")
    }
}
impl Error for FsEffectObservationError {}
