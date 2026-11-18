// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Consistency requirements for complete-file reads.

/// Consistency strength requested for a read operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::read::ReadConsistencyPolicy;
///
/// let policy = ReadConsistencyPolicy::RequireVersion;
/// assert_ne!(ReadConsistencyPolicy::BestEffort, policy);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReadConsistencyPolicy {
    /// Read the opened object without additional change detection.
    #[default]
    BestEffort,
    /// Observe changes when the provider can detect them.
    ObserveChanges,
    /// Require a version token bound to the same opened object.
    ///
    /// An open-time entity tag alone does not satisfy this policy: the read
    /// port must also establish the requested version guarantee. See
    /// [`ManagedReadPort::read_all`](crate::spi::ManagedReadPort::read_all)
    /// for the current managed read port's support boundary.
    RequireVersion,
}
