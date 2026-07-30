// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Runtime-neutral asynchronous temporary-directory facade handle.

use crate::spi::{
    AsyncTempResourceSpi,
    SpiFuture,
};
use crate::{
    AsyncFileSystem,
    AsyncTempFile,
    FsResult,
    Path,
    PersistFailure,
    PersistOptions,
    PersistOutcome,
    TempResourceState,
};

/// A facade-owned asynchronous temporary directory.
pub struct AsyncTempDirectory(
    /// Shared asynchronous temporary-resource lifecycle implementation.
    AsyncTempFile,
);

impl AsyncTempDirectory {
    /// Binds a validated provider temporary-directory session to its facade.
    ///
    /// # Parameters
    /// - `file_system`: Facade that owns validation and persistence policy.
    /// - `path`: Validated provider-local temporary path.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// An owned asynchronous temporary-directory handle.
    #[inline(always)]
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        path: Path,
        session: Box<dyn AsyncTempResourceSpi>,
    ) -> Self {
        Self(AsyncTempFile::new(file_system, path, session))
    }

    /// Returns the provider-local temporary path.
    ///
    /// # Returns
    /// The validated path supplied by the provider.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.0.path()
    }

    /// Returns the current ownership lifecycle state.
    ///
    /// # Returns
    /// The handle's current cleanup and publication state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.0.state()
    }

    /// Asynchronously confirms cleanup of this temporary directory.
    ///
    /// # Returns
    /// A future resolving after provider cleanup is confirmed.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when cleanup is no longer legal, or
    /// to the provider cleanup failure.
    #[inline(always)]
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.cleanup()
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    ///
    /// # Returns
    /// A future resolving after the provider confirms ownership transfer.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when the directory is no longer
    /// owned, or to the provider ownership-transfer failure.
    #[inline(always)]
    pub fn keep(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.keep()
    }

    /// Asynchronously persists this directory to a validated destination.
    ///
    /// # Parameters
    /// - `target`: Validated destination path.
    /// - `options`: Persistence atomicity and publication requirements.
    ///
    /// # Returns
    /// A future resolving to the confirmed persistence outcome.
    ///
    /// # Errors
    /// Resolves to a typed failure for invalid lifecycle state, failed local
    /// preflight, provider failure, or provider contract violation.
    #[inline(always)]
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        self.0.persist(target, options)
    }
}
