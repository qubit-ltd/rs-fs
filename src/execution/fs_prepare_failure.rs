// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::FsPrepareFailureKind;

/// Preparation failure retaining the exact unclaimed request.
///
/// # Type Parameters
/// `R` is the original owned request that has not been claimed for I/O.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsPrepareFailure;
///
/// fn inspect<R>(failure: &FsPrepareFailure<R>) {
///     let _reason = &failure.kind;
///     let _request = &failure.request;
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use]
pub struct FsPrepareFailure<R> {
    /// Preparation reason.
    pub kind: FsPrepareFailureKind,
    /// Original owned request.
    pub request: R,
}
impl<R> FsPrepareFailure<R> {
    /// Returns the reason and original request.
    ///
    /// # Returns
    /// The preparation reason paired with the unclaimed owned request.
    #[must_use = "handle the preparation reason and retain the unclaimed request"]
    #[inline]
    pub fn into_parts(self) -> (FsPrepareFailureKind, R) {
        (self.kind, self.request)
    }
}
