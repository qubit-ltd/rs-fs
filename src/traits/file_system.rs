// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous filesystem operations.

use crate::{
    CopyOptions,
    CopyOutcome,
    CreateDirOptions,
    DeleteOptions,
    DirectoryStream,
    FileMetadata,
    FileReader,
    FileSystemCapability,
    FileSystemProperties,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    ListOptions,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    WriteOptions,
};

/// Provider-neutral synchronous filesystem interface.
pub trait FileSystem: FileSystemProperties {
    /// Reads current metadata for a provider-local path.
    ///
    /// Implementations must inspect the final path entry itself and must not
    /// follow a final symbolic link. Metadata for a symbolic link therefore
    /// reports [`crate::FileKind::Symlink`].
    ///
    /// # Parameters
    /// - `path`: Resource path in this configured filesystem.
    ///
    /// # Returns
    /// Current provider metadata.
    ///
    /// # Errors
    /// Returns a filesystem error when metadata cannot be read.
    fn stat(&self, path: &FsPath) -> FsResult<FileMetadata>;

    /// Checks whether a path currently exists.
    ///
    /// Only an explicit [`FsErrorKind::NotFound`] maps to `false`. Permission,
    /// authentication, network, and timeout failures remain errors. This is an
    /// observation helper and must not be combined with a later mutation to
    /// emulate an atomic precondition.
    ///
    /// # Parameters
    /// - `path`: Resource path.
    ///
    /// # Returns
    /// Whether the resource was observed to exist.
    ///
    /// # Errors
    /// Returns every error other than confirmed absence.
    #[inline]
    fn exists(&self, path: &FsPath) -> FsResult<bool> {
        match self.stat(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == FsErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.with_operation(FsOperation::Exists)),
        }
    }

    /// Opens a directory, prefix, or collection enumeration.
    ///
    /// Implementations must treat [`ListOptions::page_size`] as a hint and
    /// clamp it to a finite
    /// [`FileSystemProperties::limits`](crate::FileSystemProperties::limits)
    /// `max_list_page_entries` value before issuing provider I/O. When the hint
    /// is absent, provider-selected pages must still honor that finite limit.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn list(
        &self,
        path: &FsPath,
        _options: ListOptions,
    ) -> FsResult<DirectoryStream> {
        Err(unsupported(
            self,
            path,
            FsOperation::List,
            FileSystemCapability::List,
        ))
    }

    /// Opens an already-initialized synchronous file reader.
    ///
    /// Implementations must call [`ReadOptions::validate_against`] before
    /// starting provider I/O or producing side effects.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn open_reader(
        &self,
        path: &FsPath,
        _options: ReadOptions,
    ) -> FsResult<FileReader> {
        Err(unsupported(
            self,
            path,
            FsOperation::OpenReader,
            FileSystemCapability::Read,
        ))
    }

    /// Opens an already-initialized synchronous file write session.
    ///
    /// Implementations must call [`WriteOptions::validate_against`] before
    /// creating staging resources or accepting bytes.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn open_writer(
        &self,
        path: &FsPath,
        _options: WriteOptions,
    ) -> FsResult<FileWriter> {
        Err(unsupported(
            self,
            path,
            FsOperation::OpenWriter,
            FileSystemCapability::Write,
        ))
    }

    /// Creates a directory or provider-equivalent container.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn create_dir(
        &self,
        path: &FsPath,
        _options: CreateDirOptions,
    ) -> FsResult<()> {
        Err(unsupported(
            self,
            path,
            FsOperation::CreateDir,
            FileSystemCapability::CreateDirectory,
        ))
    }

    /// Deletes a resource.
    ///
    /// Implementations must call [`DeleteOptions::validate_against`] before
    /// modifying the resource.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn delete(&self, path: &FsPath, _options: DeleteOptions) -> FsResult<()> {
        Err(unsupported(
            self,
            path,
            FsOperation::Delete,
            FileSystemCapability::Delete,
        ))
    }

    /// Renames or moves a resource within this configured filesystem.
    ///
    /// Implementations must call [`RenameOptions::validate_against`] before
    /// modifying the source or destination.
    ///
    /// # Returns
    /// The actual method and atomicity achieved.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn rename(
        &self,
        from: &FsPath,
        to: &FsPath,
        _options: RenameOptions,
    ) -> FsResult<RenameOutcome> {
        Err(unsupported(
            self,
            from,
            FsOperation::Rename,
            FileSystemCapability::Rename,
        )
        .with_target(to.clone()))
    }

    /// Copies a resource within this configured filesystem.
    ///
    /// Implementations must call [`CopyOptions::validate_against`] before
    /// starting a required server-side copy.
    ///
    /// # Returns
    /// The actual copy method and destination publication guarantee.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn copy(
        &self,
        from: &FsPath,
        to: &FsPath,
        _options: CopyOptions,
    ) -> FsResult<CopyOutcome> {
        Err(unsupported(
            self,
            from,
            FsOperation::Copy,
            FileSystemCapability::Copy,
        )
        .with_target(to.clone()))
    }

    /// Creates a provider-native or explicitly configured temporary file.
    ///
    /// Core code never invents a temporary namespace for an arbitrary backend.
    /// Providers must opt in by implementing this method.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn create_temp_file(
        &self,
        _options: TempFileOptions,
    ) -> FsResult<TempFile> {
        Err(FsError::new(
            FsErrorKind::UnsupportedCapability,
            FsOperation::CreateTemp,
            "filesystem has no configured temporary-file strategy",
        )
        .with_provider(self.info().provider_id())
        .with_required_capability(FileSystemCapability::TempFile))
    }

    /// Creates a provider-native or explicitly configured temporary directory.
    ///
    /// Core code never derives a temporary root from ordinary backend paths.
    /// Providers must opt in by implementing this method.
    ///
    /// # Errors
    /// Returns an unsupported-capability error by default.
    fn create_temp_dir(&self, _options: TempDirOptions) -> FsResult<TempDir> {
        Err(FsError::new(
            FsErrorKind::UnsupportedCapability,
            FsOperation::CreateTemp,
            "filesystem has no configured temporary-directory strategy",
        )
        .with_provider(self.info().provider_id())
        .with_required_capability(FileSystemCapability::TempDirectory))
    }
}

/// Builds a path-aware unsupported-capability error before side effects.
fn unsupported<P>(
    properties: &P,
    path: &FsPath,
    operation: FsOperation,
    capability: FileSystemCapability,
) -> FsError
where
    P: FileSystemProperties + ?Sized,
{
    FsError::new(
        FsErrorKind::UnsupportedCapability,
        operation,
        "filesystem capability is not supported",
    )
    .with_path(path.clone())
    .with_provider(properties.info().provider_id())
    .with_required_capability(capability)
}
