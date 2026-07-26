// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem operations.

use crate::{
    AsyncDirectoryStream, AsyncFileReader, AsyncFileWriter, AsyncTempDir, AsyncTempFile,
    CopyOptions, CopyOutcome, CreateDirOptions, DeleteOptions, FileMetadata, FileSystemCapability,
    FileSystemProperties, FsError, FsErrorKind, FsFuture, FsOperation, FsPath, ListOptions,
    ReadOptions, RenameOptions, RenameOutcome, TempDirOptions, TempFileOptions, WriteOptions,
};

/// Provider-neutral asynchronous filesystem interface.
pub trait AsyncFileSystem: FileSystemProperties {
    /// Asynchronously reads current metadata for a path.
    ///
    /// Implementations must inspect the final path entry itself and must not
    /// follow a final symbolic link. Metadata for a symbolic link therefore
    /// reports [`crate::FileKind::Symlink`].
    ///
    /// # Returns
    /// A future resolving to current provider metadata.
    fn stat_async<'a>(&'a self, path: &'a FsPath) -> FsFuture<'a, FileMetadata>;

    /// Asynchronously checks whether a path currently exists.
    ///
    /// Only explicit not-found maps to `false`; other failures remain errors.
    /// This observation cannot replace an atomic mutation precondition.
    ///
    /// # Returns
    /// A future resolving to the observed existence state.
    fn exists_async<'a>(&'a self, path: &'a FsPath) -> FsFuture<'a, bool> {
        Box::pin(async move {
            match self.stat_async(path).await {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == FsErrorKind::NotFound => Ok(false),
                Err(error) => Err(error.with_operation(FsOperation::Exists)),
            }
        })
    }

    /// Asynchronously opens a directory enumeration.
    ///
    /// Implementations must treat [`ListOptions::page_size`] as a hint and
    /// clamp it to a finite
    /// [`FileSystemProperties::limits`](crate::FileSystemProperties::limits)
    /// `max_list_page_entries` value before issuing provider I/O. When the hint
    /// is absent, provider-selected pages must still honor that finite limit.
    ///
    /// # Returns
    /// A future resolving to an already-open asynchronous stream.
    fn list_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ListOptions,
    ) -> FsFuture<'a, AsyncDirectoryStream> {
        let error = unsupported(path, FsOperation::List, FileSystemCapability::List);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously opens a reader and completes backend initialization.
    ///
    /// Implementations must call [`ReadOptions::validate_against`] before
    /// starting provider I/O.
    ///
    /// # Returns
    /// A future resolving to an already-open async reader.
    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader> {
        let error = unsupported(path, FsOperation::OpenReader, FileSystemCapability::Read);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously opens and initializes a provider write session.
    ///
    /// Implementations must call [`WriteOptions::validate_against`] before
    /// creating staging resources or accepting bytes.
    ///
    /// # Returns
    /// A future resolving to an already-open async writer.
    fn open_writer_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: WriteOptions,
    ) -> FsFuture<'a, AsyncFileWriter> {
        let error = unsupported(path, FsOperation::OpenWriter, FileSystemCapability::Write);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously creates a directory or provider-equivalent container.
    fn create_dir_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: CreateDirOptions,
    ) -> FsFuture<'a, ()> {
        let error = unsupported(
            path,
            FsOperation::CreateDir,
            FileSystemCapability::CreateDirectory,
        );
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously deletes a resource.
    ///
    /// Implementations must call [`DeleteOptions::validate_against`] before
    /// modifying the resource.
    fn delete_async<'a>(&'a self, path: &'a FsPath, _options: DeleteOptions) -> FsFuture<'a, ()> {
        let error = unsupported(path, FsOperation::Delete, FileSystemCapability::Delete);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously renames or moves a resource.
    ///
    /// Implementations must call [`RenameOptions::validate_against`] before
    /// modifying the source or destination.
    fn rename_async<'a>(
        &'a self,
        from: &'a FsPath,
        _to: &'a FsPath,
        _options: RenameOptions,
    ) -> FsFuture<'a, RenameOutcome> {
        let error = unsupported(from, FsOperation::Rename, FileSystemCapability::Rename);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously copies a resource within this filesystem.
    ///
    /// Implementations must call [`CopyOptions::validate_against`] before
    /// starting a required server-side copy.
    fn copy_async<'a>(
        &'a self,
        from: &'a FsPath,
        _to: &'a FsPath,
        _options: CopyOptions,
    ) -> FsFuture<'a, CopyOutcome> {
        let error = unsupported(from, FsOperation::Copy, FileSystemCapability::Copy);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously creates an explicitly supported temporary file.
    fn create_temp_file_async<'a>(
        &'a self,
        _options: TempFileOptions,
    ) -> FsFuture<'a, AsyncTempFile> {
        let error = FsError::new(
            FsErrorKind::UnsupportedCapability,
            FsOperation::CreateTemp,
            "filesystem has no configured asynchronous temporary-file strategy",
        )
        .with_required_capability(FileSystemCapability::TempFile);
        Box::pin(async move { Err(error) })
    }

    /// Asynchronously creates an explicitly supported temporary directory.
    fn create_temp_dir_async<'a>(&'a self, _options: TempDirOptions) -> FsFuture<'a, AsyncTempDir> {
        let error = FsError::new(
            FsErrorKind::UnsupportedCapability,
            FsOperation::CreateTemp,
            "filesystem has no configured asynchronous temporary-directory strategy",
        )
        .with_required_capability(FileSystemCapability::TempDirectory);
        Box::pin(async move { Err(error) })
    }
}

/// Builds a path-aware unsupported-capability error before side effects.
fn unsupported(path: &FsPath, operation: FsOperation, capability: FileSystemCapability) -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedCapability,
        operation,
        "filesystem capability is not supported",
    )
    .with_path(path.clone())
    .with_required_capability(capability)
}
