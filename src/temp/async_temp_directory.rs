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
pub struct AsyncTempDirectory(AsyncTempFile);

impl AsyncTempDirectory {
    /// Binds a validated provider temporary-directory session to its facade.
    #[inline]
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        path: Path,
        session: Box<dyn AsyncTempResourceSpi>,
    ) -> Self {
        Self(AsyncTempFile::new(file_system, path, session))
    }

    /// Returns the provider-local temporary path.
    #[inline(always)]
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.0.path()
    }

    /// Returns the current ownership lifecycle state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.0.state()
    }

    /// Asynchronously confirms cleanup of this temporary directory.
    #[inline(always)]
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.cleanup()
    }

    /// Asynchronously transfers cleanup responsibility to the caller.
    #[inline(always)]
    pub fn keep(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.keep()
    }

    /// Asynchronously persists this directory to a validated destination.
    #[inline(always)]
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        self.0.persist(target, options)
    }
}
