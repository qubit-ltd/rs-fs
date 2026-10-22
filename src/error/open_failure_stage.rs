// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stages at which an owned resource could not be opened safely.

/// Distinguishes local rejection from provider effects and invalid envelopes.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::error::OpenFailureStage;
///
/// let stage = OpenFailureStage::OutcomeValidation;
/// let recovery_may_be_retained = matches!(stage, OpenFailureStage::OutcomeValidation);
/// assert!(recovery_may_be_retained);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenFailureStage {
    /// Validation failed before dispatch; no provider I/O occurred.
    Preflight,
    /// Provider opening failed before an envelope was returned.
    ProviderOpen,
    /// An opened envelope failed facade validation; recovery may be retained.
    OutcomeValidation,
}
