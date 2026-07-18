// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-side temporary resource lifecycle sessions.

use crate::{
    FsPath,
    FsResult,
    PersistFailure,
    PersistOptions,
    PersistOutcome,
};

/// Provider lifecycle session underlying a concrete temp file or directory.
pub trait TempResourceSession: Send {
    /// Deletes the temporary source and releases staging resources.
    ///
    /// # Errors
    /// Returns a filesystem error when cleanup cannot be confirmed.
    fn cleanup(&mut self) -> FsResult<()>;

    /// Releases automatic cleanup responsibility without deleting the source.
    ///
    /// # Errors
    /// Returns a filesystem error when ownership cannot be transferred.
    fn keep(&mut self) -> FsResult<()>;

    /// Publishes the source while retaining this provider session on failure.
    ///
    /// # Parameters
    /// - `target`: Final provider-local target.
    /// - `options`: Atomicity and metadata requirements.
    ///
    /// # Returns
    /// Confirmed actual publication semantics, or a typed partial-progress
    /// failure. Implementations must never report a non-atomic success for a
    /// required-atomic request.
    fn persist(
        &mut self,
        target: &FsPath,
        options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure>;
}
