/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Factory helpers for managed temporary resources.

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
    CreateDirOptions,
    FileSystem,
    FsPath,
    FsResult,
    ManagedTempDir,
    ManagedTempFile,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    WriteMode,
    WriteOptions,
};

/// Global counter used to reduce temporary name collision risk.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Namespace for managed temporary resource creation.
pub enum TempResources {}

impl TempResources {
    /// Creates a managed temporary file.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `options`: Temporary file options.
    ///
    /// # Returns
    /// Managed temporary file handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary file cannot be reserved.
    pub fn create_file(
        fs: Arc<dyn FileSystem>,
        options: &TempFileOptions,
    ) -> FsResult<Box<dyn TempFile>> {
        let path = make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        let writer_options = WriteOptions {
            create_parent: true,
            mode: WriteMode::CreateNew,
            ..WriteOptions::default()
        };
        fs.open_writer(&path, &writer_options)?.commit()?;
        Ok(Box::new(ManagedTempFile::new(fs, path)))
    }

    /// Creates a managed temporary directory.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `options`: Temporary directory options.
    ///
    /// # Returns
    /// Managed temporary directory handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary directory cannot be
    /// created.
    pub fn create_dir(
        fs: Arc<dyn FileSystem>,
        options: &TempDirOptions,
    ) -> FsResult<Box<dyn TempDir>> {
        let path = make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        fs.create_dir(
            &path,
            &CreateDirOptions {
                recursive: true,
                ..CreateDirOptions::default()
            },
        )?;
        Ok(Box::new(ManagedTempDir::new(fs, path)))
    }
}

/// Builds a temporary resource path.
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
fn make_temp_path(parent: Option<&FsPath>, prefix: &str, suffix: &str) -> FsResult<FsPath> {
    let parent = parent.cloned().unwrap_or_else(FsPath::root);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("{prefix}{}-{nanos}-{counter}{suffix}", process::id());
    parent.join(&name)
}
