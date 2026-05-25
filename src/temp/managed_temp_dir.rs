/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Managed temporary directory implementation.

use std::sync::Arc;

use log::warn;

use crate::{
    CopyConflictPolicy,
    CopyOptions,
    DeleteOptions,
    FileSystem,
    FsErrorKind,
    FsPath,
    FsResult,
    PersistOptions,
    RenameOptions,
    TempDir,
    TempResource,
};

/// Default temporary directory backed by an [`Arc`] filesystem and path.
#[derive(Debug)]
pub struct ManagedTempDir {
    /// Filesystem that owns the temporary path.
    fs: Arc<dyn FileSystem>,
    /// Temporary path.
    path: FsPath,
    /// Whether drop should attempt best-effort cleanup.
    cleanup_on_drop: bool,
}

impl ManagedTempDir {
    /// Creates a managed temporary directory handle.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that owns the temporary path.
    /// - `path`: Temporary directory path.
    ///
    /// # Returns
    /// Managed temporary directory handle.
    #[inline]
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystem>, path: FsPath) -> Self {
        Self {
            fs,
            path,
            cleanup_on_drop: true,
        }
    }

    /// Disables drop cleanup and returns the temporary path.
    ///
    /// # Returns
    /// Retained temporary path.
    #[inline]
    fn detach(&mut self) -> FsPath {
        self.cleanup_on_drop = false;
        self.path.clone()
    }
}

impl TempResource for ManagedTempDir {
    #[inline]
    fn fs(&self) -> Arc<dyn FileSystem> {
        self.fs.clone()
    }

    #[inline]
    fn path(&self) -> &FsPath {
        &self.path
    }

    fn cleanup(mut self: Box<Self>) -> FsResult<()> {
        let result = self.fs.delete(
            &self.path,
            &DeleteOptions {
                recursive: true,
                missing_ok: true,
                if_match: None,
            },
        );
        if result.is_ok() {
            self.cleanup_on_drop = false;
        }
        result
    }

    fn keep(mut self: Box<Self>) -> FsResult<FsPath> {
        Ok(self.detach())
    }
}

impl TempDir for ManagedTempDir {
    fn persist(mut self: Box<Self>, target: &FsPath, options: &PersistOptions) -> FsResult<()> {
        let rename_options = RenameOptions {
            overwrite: options.overwrite,
            atomic: options.atomic,
        };
        match self.fs.rename(&self.path, target, &rename_options) {
            Ok(()) => {
                self.cleanup_on_drop = false;
                Ok(())
            }
            Err(error) if error.kind() == FsErrorKind::UnsupportedOperation && options.allow_copy_delete => {
                let mut copy_options = CopyOptions::tree();
                copy_options.conflict = if options.overwrite {
                    CopyConflictPolicy::Overwrite
                } else {
                    CopyConflictPolicy::Fail
                };
                copy_options.preserve_metadata = options.preserve_metadata;
                self.fs.copy(&self.path, target, &copy_options)?;
                self.fs.delete(
                    &self.path,
                    &DeleteOptions {
                        recursive: true,
                        missing_ok: true,
                        if_match: None,
                    },
                )?;
                self.cleanup_on_drop = false;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

impl Drop for ManagedTempDir {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let result = self.fs.delete(
                &self.path,
                &DeleteOptions {
                    recursive: true,
                    missing_ok: true,
                    if_match: None,
                },
            );
            if let Err(error) = result {
                warn!("failed to cleanup temporary directory '{}': {error}", self.path);
            }
        }
    }
}
