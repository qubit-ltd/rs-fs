// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Temporary directory handle trait.

use crate::{
    CreateDirOptions,
    DirectoryStream,
    FileResource,
    FsPath,
    FsResult,
    ListOptions,
    PersistOptions,
    TempResource,
};

/// Temporary directory handle with cleanup responsibility.
pub trait TempDir: TempResource {
    /// Lists child entries under the temporary directory.
    ///
    /// # Parameters
    /// - `options`: Listing options.
    ///
    /// # Returns
    /// Directory stream opened by the owning filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the owning filesystem cannot list this
    /// temporary directory.
    fn list(
        &self,
        options: &ListOptions,
    ) -> FsResult<Box<dyn DirectoryStream>> {
        self.fs().as_ref().list(self.path(), options)
    }

    /// Builds a child resource under the temporary directory.
    ///
    /// # Parameters
    /// - `name`: Provider-local child name or relative path segment.
    ///
    /// # Returns
    /// Child resource bound to the same filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the child path cannot be joined.
    fn child(&self, name: &str) -> FsResult<FileResource> {
        Ok(FileResource::new(self.fs(), self.path().join(name)?))
    }

    /// Creates and returns a child directory under this temporary directory.
    ///
    /// # Parameters
    /// - `name`: Child directory name or relative path segment.
    /// - `options`: Directory creation options.
    ///
    /// # Returns
    /// Child directory resource bound to the same filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the child path cannot be joined or the
    /// directory cannot be created.
    fn create_child_dir(
        &self,
        name: &str,
        options: &CreateDirOptions,
    ) -> FsResult<FileResource> {
        let child = self.child(name)?;
        child.create_dir(options)?;
        Ok(child)
    }

    /// Persists the temporary directory to a target path.
    ///
    /// # Parameters
    /// - `target`: Final target path.
    /// - `options`: Persistence options.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when persistence fails.
    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<()>;
}
