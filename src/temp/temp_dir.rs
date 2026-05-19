/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary directory handle trait.

use std::fmt::Debug;

use crate::{
    FsPath,
    FsResult,
    PersistOptions,
};

/// Temporary directory handle with cleanup responsibility.
pub trait TempDir: Debug + Send {
    /// Gets the temporary directory path.
    ///
    /// # Returns
    /// Provider-local path of the temporary directory.
    fn path(&self) -> &FsPath;

    /// Explicitly cleans up the temporary directory.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when cleanup fails.
    fn cleanup(self: Box<Self>) -> FsResult<()>;

    /// Persists the temporary directory to a target path.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `options`: Persistence options.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when persistence fails.
    fn persist(self: Box<Self>, target: &FsPath, options: &PersistOptions) -> FsResult<()>;

    /// Keeps the temporary directory and disables automatic cleanup.
    ///
    /// # Returns
    /// Path of the retained temporary directory.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot release cleanup
    /// responsibility.
    fn keep(self: Box<Self>) -> FsResult<FsPath>;
}
