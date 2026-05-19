/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Managed temporary file implementation.

use std::sync::Arc;

use log::warn;

use crate::{
    AtomicityRequirement,
    CopyConflictPolicy,
    CopyOptions,
    DeleteOptions,
    FileSystem,
    FsErrorKind,
    FsPath,
    FsResult,
    PersistOptions,
    RenameOptions,
    TempFile,
    WriteOutcome,
};

/// Default temporary file backed by an [`Arc`] filesystem and path.
#[derive(Debug)]
pub struct ManagedTempFile {
    /// Filesystem that owns the temporary path.
    fs: Arc<dyn FileSystem>,
    /// Temporary path.
    path: FsPath,
    /// Whether drop should attempt best-effort cleanup.
    cleanup_on_drop: bool,
}

impl ManagedTempFile {
    /// Creates a managed temporary file handle.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that owns the temporary path.
    /// - `path`: Temporary path.
    ///
    /// # Returns
    /// Managed temporary file handle.
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

impl TempFile for ManagedTempFile {
    #[inline]
    fn path(&self) -> &FsPath {
        &self.path
    }

    fn cleanup(mut self: Box<Self>) -> FsResult<()> {
        let result = self.fs.delete(
            &self.path,
            &DeleteOptions {
                missing_ok: true,
                ..DeleteOptions::default()
            },
        );
        if result.is_ok() {
            self.cleanup_on_drop = false;
        }
        result
    }

    fn persist(
        mut self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<WriteOutcome> {
        let rename_options = RenameOptions {
            overwrite: options.overwrite,
            atomic: options.atomic,
        };
        match self.fs.rename(&self.path, target, &rename_options) {
            Ok(()) => {
                self.cleanup_on_drop = false;
                Ok(WriteOutcome::default())
            }
            Err(error)
                if error.kind() == FsErrorKind::UnsupportedOperation
                    && options.allow_copy_delete =>
            {
                let mut copy_options = CopyOptions::file();
                copy_options.conflict = if options.overwrite {
                    CopyConflictPolicy::Overwrite
                } else {
                    CopyConflictPolicy::Fail
                };
                copy_options.preserve_metadata = options.preserve_metadata;
                if matches!(options.atomic, AtomicityRequirement::Required) {
                    copy_options.server_side = crate::ServerSidePreference::Require;
                }
                let outcome = self.fs.copy(&self.path, target, &copy_options)?;
                self.fs.delete(
                    &self.path,
                    &DeleteOptions {
                        missing_ok: true,
                        ..DeleteOptions::default()
                    },
                )?;
                self.cleanup_on_drop = false;
                Ok(WriteOutcome {
                    bytes_written: Some(outcome.stats.bytes),
                    etag: None,
                    diagnostics: outcome.diagnostics,
                })
            }
            Err(error) => Err(error),
        }
    }

    fn keep(mut self: Box<Self>) -> FsResult<FsPath> {
        Ok(self.detach())
    }
}

impl Drop for ManagedTempFile {
    fn drop(&mut self) {
        if self.cleanup_on_drop {
            let result = self.fs.delete(
                &self.path,
                &DeleteOptions {
                    missing_ok: true,
                    ..DeleteOptions::default()
                },
            );
            if let Err(error) = result {
                warn!("failed to cleanup temporary file '{}': {error}", self.path);
            }
        }
    }
}
