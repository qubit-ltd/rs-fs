// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
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
#[must_use]
#[non_exhaustive]
pub enum CopyAttempt {
    /// A provider completed the copy.
    Completed(
        /// Provider-confirmed completed copy outcome.
        CopyOutcome,
    ),
    /// The facade may select its later fallback only when the provider made
    /// no external change and retained no cleanup responsibility. If effects
    /// occurred or are uncertain, return [`CopyAttempt::Completed`] only for a
    /// confirmed success; otherwise return [`crate::spi::SpiCopyFailure`] with
    /// the known publication state and statistics.
    Declined(
        /// Reason the provider declined native execution.
        CopyDeclineReason,
    ),
}
