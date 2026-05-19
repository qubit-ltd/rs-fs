/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary file handle trait.

use std::fmt::Debug;

use crate::{
    FsPath,
    FsResult,
    PersistOptions,
    WriteOutcome,
};

/// Temporary file handle with cleanup responsibility.
pub trait TempFile: Debug + Send {
    /// Gets the temporary file path.
    ///
    /// # Returns
    /// Provider-local path of the temporary file.
    fn path(&self) -> &FsPath;

    /// Explicitly cleans up the temporary file.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when cleanup fails.
    fn cleanup(self: Box<Self>) -> FsResult<()>;

    /// Persists the temporary file to a target path.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `options`: Persistence options.
    ///
    /// # Returns
    /// Write outcome for the persisted resource.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when persistence fails.
    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<WriteOutcome>;

    /// Keeps the temporary file and disables automatic cleanup.
    ///
    /// # Returns
    /// Path of the retained temporary file.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot release cleanup
    /// responsibility.
    fn keep(self: Box<Self>) -> FsResult<FsPath>;
}
