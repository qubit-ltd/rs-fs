/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Common temporary resource handle trait.

use std::fmt::Debug;
use std::sync::Arc;

use crate::{
    FileMetadata,
    FileResource,
    FileSystem,
    FsPath,
    FsResult,
};

/// Common lifecycle contract for filesystem-owned temporary resources.
///
/// `TempResource` captures the behavior shared by temporary files and
/// temporary directories: both are owned by a filesystem, have a provider-local
/// path, can be queried as ordinary resources, and carry cleanup responsibility
/// until they are explicitly cleaned, persisted, or kept.
pub trait TempResource: Debug + Send + Sync {
    /// Returns the filesystem that owns this temporary resource.
    ///
    /// # Returns
    /// Shared filesystem instance that owns the temporary path.
    fn fs(&self) -> Arc<dyn FileSystem>;

    /// Returns the provider-local temporary path.
    ///
    /// # Returns
    /// Provider-local path of this temporary resource.
    fn path(&self) -> &FsPath;

    /// Converts this temporary handle to an ordinary bound file resource.
    ///
    /// # Returns
    /// A [`FileResource`] bound to the same filesystem and path.
    fn resource(&self) -> FileResource {
        FileResource::new(self.fs(), self.path().clone())
    }

    /// Checks whether the temporary resource currently exists.
    ///
    /// # Returns
    /// `true` if the owning filesystem confirms that the resource exists.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] if the owning filesystem cannot determine
    /// existence.
    fn exists(&self) -> FsResult<bool> {
        self.fs().as_ref().exists(self.path())
    }

    /// Reads metadata for the temporary resource.
    ///
    /// # Returns
    /// Metadata reported by the owning filesystem.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] if metadata cannot be read.
    fn metadata(&self) -> FsResult<FileMetadata> {
        self.fs().as_ref().path_metadata(self.path())
    }

    /// Explicitly cleans up the temporary resource.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when cleanup fails.
    fn cleanup(self: Box<Self>) -> FsResult<()>;

    /// Keeps the temporary resource and disables automatic cleanup.
    ///
    /// # Returns
    /// Path of the retained temporary resource.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the provider cannot release cleanup
    /// responsibility.
    fn keep(self: Box<Self>) -> FsResult<FsPath>;
}
