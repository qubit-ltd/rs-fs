// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Optional provider copy results.

use super::CopyDeclineReason;
use crate::copy::CopyOutcome;

/// Optional provider copy result.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::{CopyMethod, CopyOutcome, CopyStats};
/// use qubit_fs::metadata::AchievedAtomicity;
/// use qubit_fs::spi::{CopyAttempt, CopyDeclineReason};
///
/// let attempt = CopyAttempt::Completed(CopyOutcome::new(
///     CopyStats::default(),
///     CopyMethod::Native,
///     AchievedAtomicity::Atomic,
/// ));
/// assert!(matches!(attempt, CopyAttempt::Completed(_)));
/// let declined = CopyAttempt::Declined(CopyDeclineReason::NotApplicable);
/// assert!(matches!(declined, CopyAttempt::Declined(_)));
/// ```
#[non_exhaustive]
pub enum CopyAttempt {
    /// A provider completed the copy.
    Completed(
        /// Provider-confirmed completed copy outcome.
        CopyOutcome,
    ),
    /// The facade may select its later fallback.
    Declined(
        /// Reason the provider declined native execution.
        CopyDeclineReason,
    ),
}
