// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bound filesystem resource.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::sync::Arc;

use crate::{
    CopyOptions,
    CopyOutcome,
    CreateDirOptions,
    DeleteOptions,
    DirectoryStream,
    FileLocation,
    FileMetadata,
    FileReader,
    FileSystem,
    FileSystemExt,
    FileWriter,
    FsOperation,
    FsPath,
    FsResult,
    ListOptions,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    WriteOptions,
    WriteOutcome,
};

/// A filesystem path bound to the filesystem that owns it.
///
/// `FileResource` is the high-level resource object returned by
/// [`FileSystemRegistry::resource`](crate::FileSystemRegistry::resource). It
/// keeps path operations close to the resolved filesystem without making
/// [`FsPath`](crate::FsPath) itself carry any backend state.
#[derive(Clone)]
pub struct FileResource {
    fs: Arc<dyn FileSystem>,
    location: FileLocation,
}

impl FileResource {
    /// Creates a new filesystem resource.
    ///
    /// # Parameters
    /// - `fs`: Filesystem instance that owns the path.
    /// - `path`: Filesystem-local path.
    ///
    /// # Returns
    /// A resource bound to the supplied filesystem and path.
    #[inline]
    #[must_use]
    pub fn new(fs: Arc<dyn FileSystem>, path: FsPath) -> Self {
        let location = FileLocation::new(fs.info().id().clone(), path);
        Self { fs, location }
    }

    /// Creates a resource from a resolved location.
    ///
    /// # Parameters
    /// - `fs`: Filesystem that owns the location.
    /// - `location`: Provider-decoded path and optional canonical URI.
    ///
    /// # Returns
    /// A resource bound to the supplied filesystem and location.
    #[inline]
    #[must_use]
    pub fn from_location(
        fs: Arc<dyn FileSystem>,
        location: FileLocation,
    ) -> Self {
        Self { fs, location }
    }

    /// Returns the filesystem that owns this resource.
    ///
    /// # Returns
    /// A shared reference to the owning filesystem.
    #[inline(always)]
    #[must_use]
    pub fn fs(&self) -> &dyn FileSystem {
        self.fs.as_ref()
    }

    /// Returns the filesystem-local path of this resource.
    ///
    /// # Returns
    /// A shared reference to the filesystem-local path.
    #[inline(always)]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        self.location.path()
    }

    /// Returns the stable resolved resource location.
    ///
    /// # Returns
    /// Configured filesystem id, provider-local path, and optional safe URI.
    #[inline(always)]
    #[must_use]
    pub fn location(&self) -> &FileLocation {
        &self.location
    }

    /// Reads current metadata for this resource.
    ///
    /// # Returns
    /// File metadata for this resource.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot read metadata for the
    /// resource path.
    pub fn stat(&self) -> FsResult<FileMetadata> {
        self.validate_path(self.path(), FsOperation::Stat)?;
        self.fs.stat(self.path())
    }

    /// Checks whether this resource exists.
    ///
    /// # Returns
    /// `true` when the resource exists.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot determine existence
    /// for the resource path.
    pub fn exists(&self) -> FsResult<bool> {
        self.validate_path(self.path(), FsOperation::Exists)?;
        self.fs.exists(self.path())
    }

    /// Lists child entries under this resource.
    ///
    /// # Parameters
    /// - `options`: Listing options.
    ///
    /// # Returns
    /// A directory stream for the resource path.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot open a directory
    /// stream for the resource path.
    pub fn list(&self, mut options: ListOptions) -> FsResult<DirectoryStream> {
        self.validate_path(self.path(), FsOperation::List)?;
        options.page_size =
            self.fs.limits().clamp_list_page_size(options.page_size);
        self.fs.list(self.path(), options)
    }

    /// Opens this resource for reading.
    ///
    /// # Parameters
    /// - `options`: Read options.
    ///
    /// # Returns
    /// A file reader for the resource path.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot open the resource for
    /// reading.
    pub fn open_reader(&self, options: ReadOptions) -> FsResult<FileReader> {
        self.validate_path(self.path(), FsOperation::OpenReader)?;
        self.fs
            .limits()
            .validate_read_range(self.path(), options.length)?;
        options.validate_against(self.fs.capabilities())?;
        let mut reader = self.fs.open_reader(self.path(), options)?;
        reader.bind_location(self.location.clone());
        Ok(reader)
    }

    /// Opens this resource for writing.
    ///
    /// # Parameters
    /// - `options`: Write options.
    ///
    /// # Returns
    /// A file writer for the resource path.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot open the resource for
    /// writing.
    pub fn open_writer(&self, options: WriteOptions) -> FsResult<FileWriter> {
        self.validate_path(self.path(), FsOperation::OpenWriter)?;
        options.validate_against(self.fs.capabilities())?;
        let mut writer = self.fs.open_writer(self.path(), options)?;
        writer.bind_location(self.location.clone());
        Ok(writer)
    }

    /// Reads this resource into memory.
    ///
    /// # Parameters
    /// - `max_bytes`: Maximum number of bytes to retain in memory.
    ///
    /// # Returns
    /// The complete byte content of this resource.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot open or read the
    /// resource, or when it contains more than `max_bytes` bytes.
    #[inline(always)]
    pub fn read_all(&self, max_bytes: usize) -> FsResult<Vec<u8>> {
        self.fs.read_all(self.path(), max_bytes)
    }

    /// Writes all bytes to this resource.
    ///
    /// # Parameters
    /// - `bytes`: Complete byte content to write.
    ///
    /// # Returns
    /// Write outcome reported by the owning filesystem.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot open, write, flush,
    /// or commit the resource.
    #[inline(always)]
    pub fn write_all(&self, bytes: &[u8]) -> FsResult<WriteOutcome> {
        self.fs.write_all(self.path(), bytes)
    }

    /// Creates this resource as a directory.
    ///
    /// # Parameters
    /// - `options`: Directory creation options.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot create the directory.
    pub fn create_dir(&self, options: CreateDirOptions) -> FsResult<()> {
        self.validate_path(self.path(), FsOperation::CreateDir)?;
        self.fs.create_dir(self.path(), options)
    }

    /// Deletes this resource.
    ///
    /// # Parameters
    /// - `options`: Delete options.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot delete the resource.
    pub fn delete(&self, options: DeleteOptions) -> FsResult<()> {
        self.validate_path(self.path(), FsOperation::Delete)?;
        options.validate_against(self.fs.capabilities())?;
        self.fs.delete(self.path(), options)
    }

    /// Renames this resource to another filesystem-local path.
    ///
    /// # Parameters
    /// - `target`: Target filesystem-local path.
    /// - `options`: Rename options.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot rename the resource.
    pub fn rename_to(
        &self,
        target: &FsPath,
        options: RenameOptions,
    ) -> FsResult<RenameOutcome> {
        self.validate_path(self.path(), FsOperation::Rename)?;
        self.validate_path(target, FsOperation::Rename)?;
        options.validate_against(self.fs.capabilities())?;
        self.fs.rename(self.path(), target, options)
    }

    /// Copies this resource to another filesystem-local path.
    ///
    /// # Parameters
    /// - `target`: Target filesystem-local path.
    /// - `options`: Copy options.
    ///
    /// # Returns
    /// Copy outcome reported by the owning filesystem.
    ///
    /// # Errors
    /// Returns an error when the owning filesystem cannot copy the resource.
    pub fn copy_to(
        &self,
        target: &FsPath,
        options: CopyOptions,
    ) -> FsResult<CopyOutcome> {
        self.validate_path(self.path(), FsOperation::Copy)?;
        self.validate_path(target, FsOperation::Copy)?;
        options.validate_against(self.fs.capabilities())?;
        self.fs.copy(self.path(), target, options)
    }

    /// Clones the owning filesystem handle for derived resources.
    #[inline(always)]
    pub(crate) fn fs_arc(&self) -> Arc<dyn FileSystem> {
        self.fs.clone()
    }

    /// Validates `path` against the owning filesystem's declared limits.
    pub(crate) fn validate_path(
        &self,
        path: &FsPath,
        operation: FsOperation,
    ) -> FsResult<()> {
        self.fs.limits().validate_path(
            path,
            self.fs.info().path_semantics(),
            operation,
        )
    }
}

impl Debug for FileResource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("FileResource")
            .field("file_system_id", self.fs.info().id())
            .field("location", &self.location)
            .finish()
    }
}
