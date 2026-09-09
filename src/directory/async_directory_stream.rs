// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Concrete asynchronous directory stream handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::time::Instant;

use crate::directory::DirectoryStreamState;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::directory::internal::ListStreamPolicy;
use crate::error::FsResult;
use crate::metadata::DirEntry;
use crate::metadata::FileSystemLimits;
use crate::spi::AsyncDirectoryStreamSession;
use crate::spi::SpiFuture;

/// Type-erased asynchronous directory enumeration handle.
///
/// The stream validates each provider entry before yielding it and preserves
/// terminal state across asynchronous calls.
///
/// # Examples
///
/// ```
/// # use qubit_fs::{AsyncFileSystem, FsResult, Path};
/// # use qubit_fs::directory::ListOptions;
/// # async fn visit(filesystem: &AsyncFileSystem, root: &Path) -> FsResult<()> {
/// let mut stream = filesystem.list(&qubit_fs::directory::ListScope::Path(root.clone()), ListOptions::default()).await?;
/// while let Some(entry) = stream.next_entry_async().await? {
///     println!("{}", entry.path);
/// }
/// # Ok(())
/// # }
/// ```
pub struct AsyncDirectoryStream {
    /// Provider enumeration session.
    session: Box<dyn AsyncDirectoryStreamSession>,
    /// Shared validation, deadline, and terminal-state policy.
    policy: ListStreamPolicy,
}

impl AsyncDirectoryStream {
    /// Wraps an already-open asynchronous provider enumeration session.
    ///
    /// # Parameters
    /// - `session`: Provider asynchronous enumeration session.
    ///
    /// # Returns
    /// A concrete type-erased asynchronous directory stream.
    #[inline]
    pub(crate) fn new(
        scope: ListScope,
        session: Box<dyn AsyncDirectoryStreamSession>,
        options: ListOptions,
        provider: &str,
        path_semantics: crate::path::PathSemantics,
        limits: FileSystemLimits,
    ) -> FsResult<Self> {
        let policy = ListStreamPolicy::new(scope, options, provider, path_semantics, limits, Instant::now())?;
        Ok(Self { session, policy })
    }

    /// Returns the current lifecycle state of this stream.
    #[inline]
    #[must_use = "inspect the stream lifecycle state"]
    pub const fn state(&self) -> DirectoryStreamState {
        self.policy.state()
    }

    /// Asynchronously reads the next directory entry.
    ///
    /// # Returns
    /// A future resolving to one entry or `None` at end of enumeration.
    pub fn next_entry_async(&mut self) -> SpiFuture<'_, FsResult<Option<DirEntry>>> {
        Box::pin(async move {
            self.policy.before_next(Instant::now())?;
            let result = self.session.next_entry_async().await;
            self.policy.finish_next(result, Instant::now())
        })
    }
}

impl Debug for AsyncDirectoryStream {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.debug_struct("AsyncDirectoryStream").finish_non_exhaustive()
    }
}
