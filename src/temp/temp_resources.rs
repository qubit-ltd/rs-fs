/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Facade helpers for temporary resources.

use std::sync::Arc;

use crate::{
    FileSystem,
    FsResult,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
};

/// Namespace for temporary resource creation helpers.
pub enum TempResources {}

impl TempResources {
    /// Creates a temporary file.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `options`: Temporary file options.
    ///
    /// # Returns
    /// Native or managed temporary file handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when native temporary file creation fails or
    /// the managed fallback cannot reserve the temporary file.
    pub fn create_file(fs: Arc<dyn FileSystem>, options: &TempFileOptions) -> FsResult<Box<dyn TempFile>> {
        fs.temp_resource_factory().create_file(fs.clone(), options)
    }

    /// Creates a temporary file with default options.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    ///
    /// # Returns
    /// Temporary file handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary file cannot be created.
    pub fn create_default_file(fs: Arc<dyn FileSystem>) -> FsResult<Box<dyn TempFile>> {
        Self::create_file(fs, &TempFileOptions::default())
    }

    /// Creates a temporary file with a custom name prefix.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `prefix`: Name prefix.
    ///
    /// # Returns
    /// Temporary file handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary file cannot be created.
    pub fn create_file_with_prefix(fs: Arc<dyn FileSystem>, prefix: &str) -> FsResult<Box<dyn TempFile>> {
        Self::create_file(
            fs,
            &TempFileOptions {
                prefix: prefix.to_owned(),
                ..TempFileOptions::default()
            },
        )
    }

    /// Creates a temporary directory.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `options`: Temporary directory options.
    ///
    /// # Returns
    /// Native or managed temporary directory handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when native temporary directory creation fails
    /// or the managed fallback cannot create the temporary directory.
    pub fn create_dir(fs: Arc<dyn FileSystem>, options: &TempDirOptions) -> FsResult<Box<dyn TempDir>> {
        fs.temp_resource_factory().create_dir(fs.clone(), options)
    }

    /// Creates a temporary directory with default options.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    ///
    /// # Returns
    /// Temporary directory handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary directory cannot be
    /// created.
    pub fn create_default_dir(fs: Arc<dyn FileSystem>) -> FsResult<Box<dyn TempDir>> {
        Self::create_dir(fs, &TempDirOptions::default())
    }

    /// Creates a temporary directory with a custom name prefix.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that will own the temporary path.
    /// - `prefix`: Name prefix.
    ///
    /// # Returns
    /// Temporary directory handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the temporary directory cannot be
    /// created.
    pub fn create_dir_with_prefix(fs: Arc<dyn FileSystem>, prefix: &str) -> FsResult<Box<dyn TempDir>> {
        Self::create_dir(
            fs,
            &TempDirOptions {
                prefix: prefix.to_owned(),
                ..TempDirOptions::default()
            },
        )
    }
}
