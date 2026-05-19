/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Core filesystem trait.

use std::fmt::Debug;

use crate::{
    CopyOptions,
    CopyOutcome,
    CreateDirOptions,
    DeleteOptions,
    DirectoryStream,
    FileMetadata,
    FileReader,
    FileSystemCapabilities,
    FileSystemMetadata,
    FileWriter,
    FsPath,
    FsResult,
    ListOptions,
    ReadOptions,
    RenameOptions,
    WriteOptions,
};

/// Provider-neutral filesystem interface.
pub trait FileSystem: Debug + Send + Sync {
    /// Gets metadata describing this filesystem instance.
    ///
    /// # Returns
    /// Filesystem metadata.
    fn filesystem_metadata(&self) -> FileSystemMetadata;

    /// Gets capability hints for this filesystem.
    ///
    /// # Returns
    /// Static capability hints.
    #[inline]
    fn capabilities(&self) -> FileSystemCapabilities {
        self.filesystem_metadata().capabilities
    }

    /// Reads metadata for a path.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    ///
    /// # Returns
    /// Resource metadata.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when metadata cannot be read.
    fn metadata(&self, path: &FsPath) -> FsResult<FileMetadata>;

    /// Checks whether a path exists.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    ///
    /// # Returns
    /// `true` when the resource exists.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] for errors other than confirmed absence.
    fn exists(&self, path: &FsPath) -> FsResult<bool>;

    /// Lists a directory, prefix, or collection.
    ///
    /// # Parameters
    /// - `path`: Container path.
    /// - `options`: Listing options.
    ///
    /// # Returns
    /// Directory stream.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when listing cannot be started.
    fn list(&self, path: &FsPath, options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>>;

    /// Opens a readable resource.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    /// - `options`: Read options.
    ///
    /// # Returns
    /// Reader handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the reader cannot be opened.
    fn open_reader(&self, path: &FsPath, options: &ReadOptions) -> FsResult<Box<dyn FileReader>>;

    /// Opens a writable resource.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    /// - `options`: Write options.
    ///
    /// # Returns
    /// Writer handle.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the writer cannot be opened.
    fn open_writer(&self, path: &FsPath, options: &WriteOptions) -> FsResult<Box<dyn FileWriter>>;

    /// Creates a directory, collection, or equivalent container.
    ///
    /// # Parameters
    /// - `path`: Container path.
    /// - `options`: Creation options.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the container cannot be created.
    fn create_dir(&self, path: &FsPath, options: &CreateDirOptions) -> FsResult<()>;

    /// Deletes a resource.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    /// - `options`: Delete options.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when deletion fails.
    fn delete(&self, path: &FsPath, options: &DeleteOptions) -> FsResult<()>;

    /// Renames or moves a resource within this filesystem.
    ///
    /// # Parameters
    /// - `from`: Source path.
    /// - `to`: Destination path.
    /// - `options`: Rename options.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when rename fails.
    fn rename(&self, from: &FsPath, to: &FsPath, options: &RenameOptions) -> FsResult<()>;

    /// Copies a resource within this filesystem.
    ///
    /// # Parameters
    /// - `from`: Source path.
    /// - `to`: Destination path.
    /// - `options`: Copy options.
    ///
    /// # Returns
    /// Copy outcome.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when copy fails.
    fn copy(&self, from: &FsPath, to: &FsPath, options: &CopyOptions) -> FsResult<CopyOutcome>;
}
