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

/// Error returned when all bounded effect slots are reserved.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectCapacityError;
/// use qubit_fs::FsEffectSlotPool;
///
/// let pool = FsEffectSlotPool::new(0);
/// let Err(error) = pool.reserve() else { panic!("zero-capacity pool must reject reservations") };
/// assert_eq!(error, FsEffectCapacityError);
/// ```
#[must_use]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FsEffectCapacityError;
impl Display for FsEffectCapacityError {
    /// Writes the stable diagnostic for exhausted slot capacity.
    ///
    /// # Parameters
    ///
    /// - `f`: Formatter receiving the diagnostic text.
    ///
    /// # Returns
    /// The formatter result, including any formatting error.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("filesystem effect fact capacity exhausted")
    }
}
impl Error for FsEffectCapacityError {}
