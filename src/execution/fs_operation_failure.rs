// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::error::Error;
use std::fmt;

use super::FsOperationFailureKind;
use crate::error::FsEffectState;

/// Filesystem failure with the strongest known storage effect.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsOperationFailure;
///
/// fn inspect(failure: &FsOperationFailure) {
///     let _kind = &failure.kind;
///     let _effect = &failure.effect;
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub struct FsOperationFailure {
    /// Structured operation-specific failure.
    pub kind: FsOperationFailureKind,
    /// Strongest known storage effect when the operation failed.
    pub effect: FsEffectState,
}

impl FsOperationFailure {
    /// Creates a typed failure with a known storage effect.
    ///
    /// # Parameters
    /// - `kind`: Structured failure details retained by the managed operation.
    /// - `effect`: Strongest storage effect confirmed before failure.
    ///
    /// # Returns
    /// A failure value carrying both the typed cause and its storage effect.
    #[inline]
    pub fn new(kind: FsOperationFailureKind, effect: FsEffectState) -> Self {
        Self { kind, effect }
    }
}
impl fmt::Display for FsOperationFailure {
    /// Formats the operation-specific cause.
    ///
    /// # Parameters
    /// - `f`: Formatter receiving the diagnostic text.
    ///
    /// # Returns
    /// The formatter result, including any formatting error.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            FsOperationFailureKind::Provider(error) => error.fmt(f),
            FsOperationFailureKind::Copy(error) => error.fmt(f),
            FsOperationFailureKind::Write(error) => error.fmt(f),
            FsOperationFailureKind::Rename(error) => error.fmt(f),
        }
    }
}
impl Error for FsOperationFailure {
    /// Returns the retained operation-specific error cause.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FsOperationFailureKind::Provider(error) => Some(error.as_ref()),
            FsOperationFailureKind::Copy(error) => Some(error.as_ref()),
            FsOperationFailureKind::Write(error) => Some(error.as_ref()),
            FsOperationFailureKind::Rename(error) => Some(error.as_ref()),
        }
    }
}
