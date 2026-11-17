// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
/// Reason a provider could not prepare an owned request.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsPrepareFailureKind;
///
/// fn inspect(reason: &FsPrepareFailureKind) {
///     match reason {
///         FsPrepareFailureKind::Unsupported
///         | FsPrepareFailureKind::EffectCapacity
///         | FsPrepareFailureKind::ReservationInstallFailed => {}
///         FsPrepareFailureKind::Rejected(message) => {
///             let _diagnostic = message;
///         }
///     }
/// }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use]
pub enum FsPrepareFailureKind {
    /// Operation is unsupported.
    Unsupported,
    /// Request was rejected before I/O with a bounded diagnostic.
    Rejected(
        /// Bounded diagnostic explaining why the provider rejected the request.
        String,
    ),
    /// No bounded mutation fact slot was available before preparation.
    EffectCapacity,
    /// Provider did not accept its reserved fact slot before submission.
    ReservationInstallFailed,
}
