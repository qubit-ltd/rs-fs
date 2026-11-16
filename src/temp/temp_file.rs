// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-backed temporary file lifecycle handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::FileSystem;
use crate::error::FsResult;
use crate::path::Path;
use crate::spi::TempResourceSpi;
use crate::temp::PersistFailure;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;
use crate::temp::internal::SyncTempResource;

/// Temporary file retaining the provider session until its lifecycle completes.
///
/// Dropping this synchronous handle while its state is
/// [`TempResourceState::Owned`] or [`TempResourceState::CleanupRequired`] calls
/// provider cleanup synchronously on the current thread. Cleanup may perform
/// I/O and block, and any cleanup error is ignored. Call [`Self::cleanup`]
/// explicitly when the result matters; if it fails and leaves the state as
/// `CleanupRequired`, dropping the handle may make another best-effort cleanup
/// attempt. The facade does not call cleanup on drop in the `Cleaned`,
/// `Persisted`, `Kept`, or `Indeterminate` states.
///
/// This contract applies to synchronous handles. Async handles use their
/// separate `cancel_on_drop` behavior, and rejected handles use their isolated
/// recovery contract; neither implies synchronous cleanup by dropping this
/// type.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FileSystem;
/// use qubit_fs::temp::{TempOptions, TempResourceState};
///
/// fn create_and_cleanup(filesystem: &FileSystem) -> Result<(), Box<dyn std::error::Error>> {
///     let mut temporary = filesystem.create_temp_file(TempOptions::default())?;
///     assert_eq!(TempResourceState::Owned, temporary.state());
///     temporary.cleanup()?;
///     assert_eq!(TempResourceState::Cleaned, temporary.state());
///     Ok(())
/// }
/// ```
pub struct TempFile {
    /// Shared synchronous resource lifecycle.
    core: SyncTempResource,
}

impl TempFile {
    /// Creates the facade handle from validated provider parts.
    ///
    /// # Parameters
    /// - `filesystem`: Facade that owns validation and persistence policy.
    /// - `path`: Validated provider-local temporary file path.
    /// - `session`: Provider lifecycle session for this file.
    ///
    /// # Returns
    /// An owned temporary-file handle.
    pub(crate) fn new(filesystem: FileSystem, path: Path, session: Box<dyn TempResourceSpi>) -> Self {
        Self {
            core: SyncTempResource::new(
                filesystem,
                path,
                session,
                "temporary file cannot perform this lifecycle operation",
            ),
        }
    }

    /// Returns the temporary logical path.
    ///
    /// # Returns
    /// The provider-local path retained by this handle.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.core.path()
    }

    /// Returns the resource lifecycle state.
    ///
    /// # Returns
    /// The current ownership and cleanup state.
    #[inline]
    pub const fn state(&self) -> TempResourceState {
        self.core.state()
    }

    /// Persists this temporary file to `target`.
    ///
    /// # Parameters
    /// - `target`: Destination path validated by the owning filesystem.
    /// - `options`: Atomicity and publication requirements.
    ///
    /// # Returns
    /// The provider-confirmed publication outcome.
    ///
    /// # Errors
    /// Returns a typed failure for invalid lifecycle state, failed preflight,
    /// provider failure, or a provider contract violation.
    #[allow(clippy::result_large_err)]
    pub fn persist(&mut self, target: &Path, options: PersistOptions) -> Result<PersistOutcome, PersistFailure> {
        self.core.persist(target, options)
    }

    /// Publishes this temporary file to the provider-generated target.
    ///
    /// # Returns
    /// The provider-confirmed publication outcome and generated target.
    ///
    /// # Errors
    /// Returns a typed failure when ownership is unavailable, provider
    /// publication fails, or the provider returns an invalid target.
    #[allow(clippy::result_large_err)]
    pub fn keep(&mut self) -> Result<PersistOutcome, PersistFailure> {
        self.core.keep()
    }

    /// Cleans the source and releases the session responsibility.
    ///
    /// Call this method when cleanup success or failure must be observed.
    /// Unlike dropping an owned handle, this returns the provider result. If
    /// cleanup fails while the state remains `CleanupRequired`, a later drop
    /// may make another best-effort attempt.
    ///
    /// # Errors
    /// Returns an invalid-state error when cleanup is no longer legal, or the
    /// provider cleanup error when cleanup cannot be confirmed.
    pub fn cleanup(&mut self) -> FsResult<()> {
        self.core.cleanup()
    }
}

impl Debug for TempFile {
    /// Formats the handle path and lifecycle state without exposing provider
    /// internals.
    ///
    /// # Parameters
    /// - `f`: Formatter that receives the debug representation.
    ///
    /// # Returns
    /// `Ok(())` when formatting succeeds, or the formatter's error.
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("TempFile")
            .field("path", self.core.path())
            .field("state", &self.core.state())
            .finish_non_exhaustive()
    }
}
