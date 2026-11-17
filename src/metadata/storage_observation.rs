// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Point-in-time storage capacity observations.

/// Point-in-time storage capacity observation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::StorageObservation;
///
/// let observation = StorageObservation::Known {
///     available_bytes: 1024,
///     total_bytes: 2048,
/// };
/// assert!(matches!(observation, StorageObservation::Known { .. }));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum StorageObservation {
    /// Available and total capacity are known in bytes.
    Known {
        /// Available capacity.
        available_bytes: u64,
        /// Total capacity.
        total_bytes: u64,
    },
    /// The provider cannot report capacity.
    Unknown,
}
