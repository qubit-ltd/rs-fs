/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Temporary resource factory trait.

use std::fmt::Debug;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicU64,
    Ordering,
};
use std::time::{
    SystemTime,
    UNIX_EPOCH,
};

use crate::{
    FileSystem,
    FsPath,
    FsResult,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
};

/// Global counter used to reduce temporary name collision risk.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Factory for filesystem-owned temporary resources.
pub trait TempResourceFactory: Debug + Send + Sync {
    /// Creates a temporary file.
    ///
    /// # Parameters
    /// - `owner`: Filesystem that will own the temporary file.
    /// - `options`: Temporary file creation options.
    ///
    /// # Returns
    /// Temporary file handle with cleanup responsibility.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary file cannot be created.
    fn create_file(&self, owner: Arc<dyn FileSystem>, options: &TempFileOptions) -> FsResult<Box<dyn TempFile>>;

    /// Creates a temporary directory.
    ///
    /// # Parameters
    /// - `owner`: Filesystem that will own the temporary directory.
    /// - `options`: Temporary directory creation options.
    ///
    /// # Returns
    /// Temporary directory handle with cleanup responsibility.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary directory cannot be
    /// created.
    fn create_dir(&self, owner: Arc<dyn FileSystem>, options: &TempDirOptions) -> FsResult<Box<dyn TempDir>>;

    /// Builds a temporary resource path using the common rs-fs naming format.
    ///
    /// The default format is `{prefix}{process_id}-{unix_epoch_nanos}-{counter}{suffix}`.
    /// Implementations may use this helper to share the common format, or ignore
    /// it and apply provider-specific naming rules.
    ///
    /// # Parameters
    /// - `parent`: Optional parent path. Root is used when absent.
    /// - `prefix`: Temporary name prefix.
    /// - `suffix`: Temporary name suffix.
    ///
    /// # Returns
    /// Generated temporary path.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the generated path is invalid.
    fn make_temp_path(&self, parent: Option<&FsPath>, prefix: &str, suffix: &str) -> FsResult<FsPath> {
        let parent = parent.cloned().unwrap_or_else(FsPath::root);
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!("{prefix}{}-{nanos}-{counter}{suffix}", process::id());
        parent.join(&name)
    }
}
