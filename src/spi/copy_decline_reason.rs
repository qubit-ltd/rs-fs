// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Provider copy-decline reasons.

/// Reason a provider declined its optional copy fast path.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::spi::{CopyAttempt, CopyDeclineReason};
///
/// let attempt = CopyAttempt::Declined(CopyDeclineReason::NotApplicable);
/// assert!(matches!(
///     attempt,
///     CopyAttempt::Declined(CopyDeclineReason::NotApplicable)
/// ));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CopyDeclineReason {
    /// The provider does not implement a copy primitive.
    NotImplemented,
    /// The provider primitive cannot safely serve this particular request.
    NotApplicable,
}
