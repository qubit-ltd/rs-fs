// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Bound asynchronous filesystem resources.

use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::sync::Arc;

use crate::{
    AsyncDirectoryStream, AsyncFileReader, AsyncFileSystem, AsyncFileSystemExt, AsyncFileWriter,
    CopyOptions, CopyOutcome, CreateDirOptions, DeleteOptions, FileLocation, FileMetadata,
    FsFuture, FsOperation, FsPath, FsResult, FsUri, ListOptions, ReadOptions, RenameOptions,
    RenameOutcome, WriteOptions, WriteOutcome,
};

/// A provider-local path bound to an asynchronous filesystem object.
#[derive(Clone)]
pub struct AsyncFileResource {
    fs: Arc<dyn AsyncFileSystem>,
    location: FileLocation,
}

impl AsyncFileResource {
    /// Creates a bound asynchronous resource from a provider-local path.
    #[inline]
    #[must_use]
    pub fn new(fs: Arc<dyn AsyncFileSystem>, path: FsPath) -> Self {
        let location = FileLocation::new(fs.info().id().clone(), path);
        Self { fs, location }
    }

    /// Creates a resource from a resolved path and canonical URI.
    ///
    /// # Parameters
    /// - `fs`: Asynchronous filesystem that owns the path.
    /// - `path`: Provider-decoded path.
    /// - `canonical_uri`: Canonical URI used to resolve the resource.
    ///
    /// # Returns
    /// A resource whose location identity is derived from `fs`.
    #[inline]
    #[must_use]
    pub fn from_resolved(fs: Arc<dyn AsyncFileSystem>, path: FsPath, canonical_uri: FsUri) -> Self {
        let location = FileLocation::new(fs.info().id().clone(), path).with_uri(canonical_uri);
        Self { fs, location }
    }

    /// Returns the owning asynchronous filesystem.
    #[inline(always)]
    #[must_use]
    pub fn fs(&self) -> &dyn AsyncFileSystem {
        self.fs.as_ref()
    }

    /// Returns the provider-local path.
    #[inline(always)]
    #[must_use]
    pub fn path(&self) -> &FsPath {
        self.location.path()
    }

    /// Returns the resolved identity and optional safe URI.
    #[inline(always)]
    #[must_use]
    pub fn location(&self) -> &FileLocation {
        &self.location
    }

    /// Asynchronously reads current metadata.
    #[inline]
    pub fn stat_async(&self) -> FsFuture<'_, FileMetadata> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::Stat) {
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .stat_async(self.path())
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously checks observed existence.
    #[inline]
    pub fn exists_async(&self) -> FsFuture<'_, bool> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::Exists) {
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .exists_async(self.path())
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously opens a directory enumeration.
    #[inline]
    pub fn list_async(&self, mut options: ListOptions) -> FsFuture<'_, AsyncDirectoryStream> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::List) {
            return Box::pin(async move { Err(error) });
        }
        options.page_size = self.fs.limits().clamp_list_page_size(options.page_size);
        Box::pin(async move {
            self.fs
                .list_async(self.path(), options)
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously opens an already-initialized file reader.
    #[inline]
    pub fn open_reader_async(&self, options: ReadOptions) -> FsFuture<'_, AsyncFileReader> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::OpenReader) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = self
            .fs
            .limits()
            .validate_read_range(self.path(), options.length)
        {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = options.validate_against(self.fs.capabilities()) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        let location = self.location.clone();
        Box::pin(async move {
            let mut reader = self
                .fs
                .open_reader_async(self.path(), options)
                .await
                .map_err(|error| self.with_context(error, None))?;
            reader.bind_location(location);
            Ok(reader)
        })
    }

    /// Asynchronously opens an already-initialized file writer.
    #[inline]
    pub fn open_writer_async(&self, options: WriteOptions) -> FsFuture<'_, AsyncFileWriter> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::OpenWriter) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = options.validate_against(self.fs.capabilities()) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        let location = self.location.clone();
        Box::pin(async move {
            let mut writer = self
                .fs
                .open_writer_async(self.path(), options)
                .await
                .map_err(|error| self.with_context(error, None))?;
            writer.bind_location(location);
            Ok(writer)
        })
    }

    /// Asynchronously reads this resource into memory.
    ///
    /// # Parameters
    ///
    /// - `max_bytes`: Maximum number of bytes to retain in memory.
    ///
    /// # Returns
    ///
    /// A future resolving to the complete byte content.
    ///
    /// # Errors
    ///
    /// The future resolves to an error when the owning filesystem cannot open
    /// or read the resource, or when it contains more than `max_bytes` bytes.
    #[inline(always)]
    pub fn read_all_async(&self, max_bytes: usize) -> FsFuture<'_, Vec<u8>> {
        Box::pin(async move {
            self.fs
                .read_all_async(self.path(), max_bytes)
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously writes and commits complete byte content.
    ///
    /// # Parameters
    ///
    /// - `bytes`: Complete byte content to write.
    ///
    /// # Returns
    ///
    /// A future resolving to the provider publication outcome.
    ///
    /// # Errors
    ///
    /// The future resolves to an error when the owning filesystem cannot open,
    /// write, abort, or commit the resource, or when a declared finite provider
    /// limit is exceeded.
    #[inline(always)]
    pub fn write_all_async<'a>(&'a self, bytes: &'a [u8]) -> FsFuture<'a, WriteOutcome> {
        Box::pin(async move {
            self.fs
                .write_all_async(self.path(), bytes)
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously creates this resource as a directory.
    #[inline]
    pub fn create_dir_async(&self, options: CreateDirOptions) -> FsFuture<'_, ()> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::CreateDir) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .create_dir_async(self.path(), options)
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously deletes this resource.
    #[inline]
    pub fn delete_async(&self, options: DeleteOptions) -> FsFuture<'_, ()> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::Delete) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = options.validate_against(self.fs.capabilities()) {
            let error = self.with_context(error, None);
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .delete_async(self.path(), options)
                .await
                .map_err(|error| self.with_context(error, None))
        })
    }

    /// Asynchronously renames this resource.
    #[inline]
    pub fn rename_to_async<'a>(
        &'a self,
        target: &'a FsPath,
        options: RenameOptions,
    ) -> FsFuture<'a, RenameOutcome> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::Rename) {
            let error = self.with_context(error, Some(target));
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = self.validate_target_path(target, FsOperation::Rename) {
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = options.validate_against(self.fs.capabilities()) {
            let error = self.with_context(error, Some(target));
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .rename_async(self.path(), target, options)
                .await
                .map_err(|error| self.with_context(error, Some(target)))
        })
    }

    /// Asynchronously copies this resource.
    #[inline]
    pub fn copy_to_async<'a>(
        &'a self,
        target: &'a FsPath,
        options: CopyOptions,
    ) -> FsFuture<'a, CopyOutcome> {
        if let Err(error) = self.validate_path(self.path(), FsOperation::Copy) {
            let error = self.with_context(error, Some(target));
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = self.validate_target_path(target, FsOperation::Copy) {
            return Box::pin(async move { Err(error) });
        }
        if let Err(error) = options.validate_against(self.fs.capabilities()) {
            let error = self.with_context(error, Some(target));
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            self.fs
                .copy_async(self.path(), target, options)
                .await
                .map_err(|error| self.with_context(error, Some(target)))
        })
    }

    /// Clones the owning filesystem for derived resources.
    #[inline(always)]
    pub(crate) fn fs_arc(&self) -> Arc<dyn AsyncFileSystem> {
        self.fs.clone()
    }

    /// Validates `path` against the owning filesystem's declared limits.
    pub(crate) fn validate_path(&self, path: &FsPath, operation: FsOperation) -> FsResult<()> {
        self.fs
            .limits()
            .validate_path(path, self.fs.info().path_semantics(), operation)
            .map_err(|error| self.with_context(error, None))
    }

    /// Validates a destination path while retaining source and target roles.
    fn validate_target_path(&self, target: &FsPath, operation: FsOperation) -> FsResult<()> {
        self.fs
            .limits()
            .validate_path(target, self.fs.info().path_semantics(), operation)
            .map_err(|error| {
                self.with_context(
                    error
                        .with_path(self.path().clone())
                        .with_target(target.clone()),
                    Some(target),
                )
            })
    }

    /// Adds resource identity to an error that crossed this abstraction
    /// boundary without replacing provider-supplied context.
    fn with_context(&self, error: crate::FsError, target: Option<&FsPath>) -> crate::FsError {
        error.with_missing_context(self.path(), target, self.fs.info().provider_id())
    }
}

impl Debug for AsyncFileResource {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("AsyncFileResource")
            .field("location", &self.location)
            .finish()
    }
}
