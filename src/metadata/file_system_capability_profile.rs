// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Per-mode capability snapshots for a configured filesystem.

use super::FileSystemCapabilities;

/// Per-mode capability snapshot for a configured filesystem provider.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{FileSystemCapabilities, FileSystemCapabilityProfile};
///
/// let empty = FileSystemCapabilities::new();
/// let profile = FileSystemCapabilityProfile::new(empty, empty);
/// assert_eq!(empty, profile.synchronous);
/// assert_eq!(empty, profile.asynchronous);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FileSystemCapabilityProfile {
    /// Capability strengths for synchronous provider paths.
    pub synchronous: FileSystemCapabilities,
    /// Capability strengths for asynchronous provider paths.
    pub asynchronous: FileSystemCapabilities,
}
impl FileSystemCapabilityProfile {
    /// Derives independent conditional capability snapshots from sync and
    /// async provider entry points.
    ///
    /// Operation presence indicates that the operation can be attempted. It
    /// does not imply a guarantee for every request.
    ///
    /// # Parameters
    /// - `synchronous`: Operations exposed by the synchronous provider path.
    /// - `asynchronous`: Operations exposed by the asynchronous provider path.
    ///
    /// # Returns
    /// A profile containing each path's conditional capability snapshot.
    #[must_use]
    pub fn from_operations(
        synchronous: crate::spi::ProviderOperations,
        asynchronous: crate::spi::ProviderOperations,
    ) -> Self {
        Self::new(
            synchronous.conditional_capabilities(),
            asynchronous.conditional_capabilities(),
        )
    }

    /// Creates a profile with independently supplied sync and async snapshots.
    ///
    /// # Parameters
    /// - `synchronous`: Capability snapshot for synchronous provider calls.
    /// - `asynchronous`: Capability snapshot for asynchronous provider calls.
    ///
    /// # Returns
    /// A profile containing the supplied snapshots.
    pub const fn new(synchronous: FileSystemCapabilities, asynchronous: FileSystemCapabilities) -> Self {
        Self {
            synchronous,
            asynchronous,
        }
    }
}
