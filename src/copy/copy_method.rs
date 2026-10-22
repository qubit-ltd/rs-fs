// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy method model.

/// Method used to complete a copy operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::CopyMethod;
/// use qubit_fs::copy::CopyOutcome;
/// use qubit_fs::copy::CopyStats;
/// use qubit_fs::metadata::AchievedAtomicity;
///
/// let outcome = CopyOutcome::new(
///     CopyStats::default(),
///     CopyMethod::Streamed,
///     AchievedAtomicity::Atomic,
/// );
/// assert_eq!(outcome.method(), CopyMethod::Streamed);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyMethod {
    /// Provider-native copy.
    Native,
    /// Storage-level clone or reflink copy.
    Clone,
    /// Server-side copy.
    ServerSide,
    /// Client-side stream copy.
    Streamed,
    /// Mixed copy strategy.
    Mixed,
}
