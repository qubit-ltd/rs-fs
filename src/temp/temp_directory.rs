// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-backed temporary directory lifecycle handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::FileSystem;
use crate::error::FsResult;
use crate::path::Path;
use crate::path::PathComponent;
use crate::path::RelativePath;
use crate::spi::TempResourceSpi;
use crate::temp::PersistFailure;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;
use crate::temp::internal::SyncTempResource;

/// Temporary directory retaining the provider session until lifecycle
/// completion.
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
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # let filesystem = support::rustdoc_provider::filesystem();
/// use qubit_fs::temp::{TempOptions, TempResourceState};
///
/// let mut temporary = filesystem.create_temp_directory(TempOptions::default())?;
/// assert_eq!(TempResourceState::Owned, temporary.state());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct TempDirectory {
    /// Shared synchronous resource lifecycle.
    core: SyncTempResource,
}
impl TempDirectory {
    /// Creates the facade handle from validated provider parts.
    ///
    /// # Parameters
    /// - `filesystem`: Facade that validates and applies persistence policy.
    /// - `path`: Provider-local path of the opened temporary directory.
    /// - `session`: Provider session that owns the directory lifecycle.
    ///
    /// # Returns
    /// A handle initialized with cleanup ownership of the opened directory.
    pub(crate) fn new(filesystem: FileSystem, path: Path, session: Box<dyn TempResourceSpi>) -> Self {
        Self {
            core: SyncTempResource::new(
                filesystem,
                path,
                session,
                "temporary directory cannot perform this lifecycle operation",
            ),
        }
    }

    /// Returns the logical temporary directory path.
    ///
    /// # Returns
    /// The provider-local logical path while this handle is borrowed.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.core.path()
    }

    /// Returns the resource lifecycle state.
    ///
    /// # Returns
    /// The current ownership and cleanup state of this temporary resource.
    #[inline]
    pub const fn state(&self) -> TempResourceState {
        self.core.state()
    }

    /// Returns one lexically safe child path.
    ///
    /// # Parameters
    /// - `component`: A validated single path component.
    ///
    /// # Returns
    /// The component appended to this temporary directory's logical path.
    #[inline]
    #[must_use]
    pub fn child(&self, component: &PathComponent) -> Path {
        self.core.path().child(component)
    }

    /// Returns one lexically safe descendant path.
    ///
    /// # Parameters
    /// - `relative`: A validated relative path that cannot escape this
    ///   directory.
    ///
    /// # Returns
    /// The relative path joined to this temporary directory's logical path.
    #[inline]
    #[must_use]
    pub fn descendant(&self, relative: &RelativePath) -> Path {
        self.core.path().join(relative)
    }

    /// Persists this directory.
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

    /// Publishes this temporary directory to the provider-generated target.
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

    /// Cleans the temporary directory.
    ///
    /// Call this method when cleanup success or failure must be observed.
    /// Unlike dropping an owned handle, this returns the provider result. If
    /// cleanup fails while the state remains `CleanupRequired`, a later drop
    /// may make another best-effort attempt.
    ///
    /// # Errors
    /// Returns an invalid-state error when cleanup is no longer legal, or the
    /// provider cleanup error when cleanup cannot be confirmed.
    ///
    /// # Returns
    /// `Ok(())` when cleanup is confirmed; otherwise, the contextual cleanup
    /// error.
    pub fn cleanup(&mut self) -> FsResult<()> {
        self.core.cleanup()
    }
}
impl Debug for TempDirectory {
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
        f.debug_struct("TempDirectory")
            .field("path", self.core.path())
            .field("state", &self.core.state())
            .finish_non_exhaustive()
    }
}
