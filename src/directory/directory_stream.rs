// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Concrete synchronous directory stream handle.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::time::Instant;

use crate::directory::DirectoryStreamState;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::directory::ListStreamPolicy;
use crate::error::FsResult;
use crate::metadata::DirEntry;
use crate::metadata::FileSystemLimits;
use crate::path::PathConstraints;
use crate::path::PathSemantics;
use crate::spi::DirectoryStreamSpi;

/// Type-erased synchronous directory enumeration handle.
///
/// The stream validates every provider entry against the requested root and
/// listing options before returning it to the caller.
///
/// # Examples
///
/// This helper demonstrates the normal bounded-listing workflow without
/// requiring a concrete provider in the documentation build.
///
/// ```
/// # use qubit_fs::{FileSystem, FsResult, Path};
/// # use qubit_fs::directory::ListOptions;
/// # fn visit(filesystem: &FileSystem, root: &Path) -> FsResult<()> {
/// let mut stream = filesystem.list(
///     &qubit_fs::directory::ListScope::Path(root.clone()),
///     ListOptions::default(),
/// )?;
/// while let Some(entry) = stream.next_entry()? {
///     println!("{}", entry.path);
/// }
/// # Ok(())
/// # }
/// ```
pub struct DirectoryStream {
    /// Provider enumeration session.
    session: Box<dyn DirectoryStreamSpi>,
    /// Shared validation, deadline, and terminal-state policy.
    policy: ListStreamPolicy,
}

impl DirectoryStream {
    /// Wraps an already-open provider enumeration session.
    ///
    /// # Parameters
    /// - `scope`: Root and scope that constrain the entries returned by the
    ///   provider.
    /// - `session`: Provider directory enumeration session.
    /// - `options`: Filters and resource bounds applied to provider entries.
    /// - `provider`: Provider identifier attached to stream errors.
    /// - `path_semantics`: Provider path rules used to validate returned
    ///   entries.
    /// - `limits`: Provider limits used to validate returned entries.
    /// - `constraints`: Provider path-form constraints used to validate
    ///   returned entries.
    ///
    /// # Returns
    /// A concrete type-erased stream that validates entries and enforces the
    /// configured listing bounds.
    ///
    /// # Errors
    /// Returns an invalid-options error if the configured listing deadline
    /// cannot be represented by the platform's monotonic clock.
    #[inline]
    pub(crate) fn new(
        scope: ListScope,
        session: Box<dyn DirectoryStreamSpi>,
        options: ListOptions,
        provider: &str,
        path_semantics: PathSemantics,
        limits: FileSystemLimits,
        constraints: PathConstraints,
    ) -> FsResult<Self> {
        let policy = ListStreamPolicy::new(
            scope,
            options,
            provider,
            path_semantics,
            limits,
            constraints,
            Instant::now(),
        )?;
        Ok(Self { session, policy })
    }

    /// Returns the current lifecycle state of this stream.
    #[inline]
    #[must_use = "inspect the stream lifecycle state"]
    pub const fn state(&self) -> DirectoryStreamState {
        self.policy.state()
    }

    /// Reads the next directory entry.
    ///
    /// # Returns
    /// `Some` for one entry or `None` at the first end-of-file observation.
    ///
    /// Once `None` has been returned, the stream is terminal and this method
    /// returns `InvalidState` on later calls.
    ///
    /// # Errors
    /// Returns a filesystem error when enumeration cannot continue. After an
    /// error the stream is terminal; reopen it to enumerate again.
    #[inline]
    pub fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        self.policy.before_next(Instant::now())?;
        let result = self.session.next_entry();
        self.policy.finish_next(result, Instant::now())
    }
}

impl Debug for DirectoryStream {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.debug_struct("DirectoryStream").finish_non_exhaustive()
    }
}
