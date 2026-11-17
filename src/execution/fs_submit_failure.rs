// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_operation::AdmissionError;

use super::FsPrepareFailureKind;

/// Submission failure retaining the original owned request.
///
/// # Type Parameters
/// `R` is the original owned request that was not submitted successfully.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsSubmitFailure;
///
/// fn inspect<R>(failure: &FsSubmitFailure<R>) {
///     let _admission = &failure.admission;
///     let _preparation = &failure.preparation;
///     let _request = &failure.request;
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub struct FsSubmitFailure<R> {
    /// Admission error, if submission was attempted.
    pub admission: Option<AdmissionError>,
    /// Preparation or binding error, if preparation failed.
    pub preparation: Option<FsPrepareFailureKind>,
    /// Original owned request.
    pub request: R,
}
impl<R> FsSubmitFailure<R> {
    /// Returns failure details and original owned request.
    ///
    /// # Returns
    /// Admission and preparation failure details paired with the original
    /// request.
    #[must_use = "handle the admission or preparation failure and retain the request"]
    #[inline]
    pub fn into_parts(self) -> (Option<AdmissionError>, Option<FsPrepareFailureKind>, R) {
        (self.admission, self.preparation, self.request)
    }
}
