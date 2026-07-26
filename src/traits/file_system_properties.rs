// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Properties shared by synchronous and asynchronous filesystems.

use crate::{FileSystemCapabilities, FileSystemInfo, FileSystemLimits};

/// Construction-time local snapshots shared by all filesystem operation modes.
pub trait FileSystemProperties: Send + Sync {
    /// Returns immutable identity and provider information.
    ///
    /// This getter must not perform local or remote I/O.
    ///
    /// # Returns
    /// Information fixed when the configured filesystem was constructed.
    fn info(&self) -> &FileSystemInfo;

    /// Returns stable capability guarantees for this configured filesystem.
    ///
    /// This getter must not perform I/O. A provider that needs remote probing
    /// must complete it during construction.
    ///
    /// # Returns
    /// Stable capability guarantees.
    fn capabilities(&self) -> FileSystemCapabilities;

    /// Returns stable provider-declared filesystem limits.
    ///
    /// This getter must not perform I/O. Unknown, inapplicable, and unbounded
    /// dimensions must be represented explicitly in the returned snapshot.
    ///
    /// # Returns
    /// Limits fixed when the configured filesystem was constructed.
    fn limits(&self) -> &FileSystemLimits;
}
