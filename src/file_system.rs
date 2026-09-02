// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validating synchronous filesystem facade.

use std::sync::Arc;

use crate::copy::CopyFailure;
use crate::copy::CopyOperation;
use crate::copy::CopyOptions;
use crate::copy::CopyOutcome;
use crate::directory::CreateDirectoryOptions;
use crate::directory::CreateDirectoryOutcome;
use crate::directory::DeleteOptions;
use crate::directory::DeleteOutcome;
use crate::directory::DirectoryOperation;
use crate::directory::DirectoryStream;
use crate::directory::ListOptions;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::path::Path;
use crate::read::FileReader;
use crate::read::ReadOperation;
use crate::read::ReadOptions;
use crate::rename::RenameFailure;
use crate::rename::RenameFailureState;
use crate::rename::RenameOptions;
use crate::rename::RenameOutcome;
use crate::rename::validate_rename_outcome;
use crate::spi::CreateDirectoryRequest;
use crate::spi::CreateTempDirectoryRequest;
use crate::spi::CreateTempFileRequest;
use crate::spi::DeleteDirectoryRequest;
use crate::spi::DeleteFileRequest;
use crate::spi::FileSystemSpi;
use crate::spi::OpenReaderRequest;
use crate::spi::OpenWriterRequest;
use crate::spi::RenameRequest;
use crate::spi::ResolvedCreateDirectoryOptions;
use crate::spi::ResolvedDeleteOptions;
use crate::spi::ResolvedReadOptions;
use crate::spi::ResolvedRenameOptions;
use crate::spi::ResolvedWriteOptions;
use crate::spi::StatRequest;
use crate::temp::TempDirectory;
use crate::temp::TempFile;
use crate::temp::TempOptions;
use crate::write::FileWriter;
use crate::write::WriteAllFailure;
use crate::write::WriteOperation;
use crate::write::WriteOptions;

/// Application-facing synchronous filesystem facade.
#[derive(Clone)]
pub struct FileSystem {
    /// Provider implementation receiving validated synchronous requests.
    spi: Arc<dyn FileSystemSpi>,
    /// Shared immutable state and deterministic preflight policy.
    core: Arc<FacadeCore>,
}

impl FileSystem {
    /// Constructs a facade and caches the provider's single validated property
    /// snapshot.
    #[inline]
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: FileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    #[inline]
    pub fn from_shared_spi(spi: Arc<dyn FileSystemSpi>) -> FsResult<Self> {
        let core = FacadeCore::new(spi.properties())?;
        Ok(Self {
            spi,
            core: Arc::new(core),
        })
    }

    /// Returns the immutable property snapshot without provider I/O.
    #[inline(always)]
    #[must_use]
    pub fn properties(&self) -> &FileSystemProperties {
        self.core.properties()
    }

    /// Returns shared deterministic facade policy to operation objects.
    #[inline(always)]
    pub(crate) fn core(&self) -> &FacadeCore {
        &self.core
    }

    /// Returns the synchronous provider implementation to operation objects.
    #[inline(always)]
    pub(crate) fn spi(&self) -> &dyn FileSystemSpi {
        self.spi.as_ref()
    }

    /// Reads metadata after all local validation succeeds.
    pub fn stat(&self, path: &Path) -> FsResult<FileMetadata> {
        self.validate_path(path, FsOperation::Stat)?;
        let response = self
            .spi
            .stat(StatRequest::new(path, ()))
            .map_err(|error| self.enrich(error, path, FsOperation::Stat))?;
        if response.path() != path {
            return Err(self.contract_error(
                path,
                FsOperation::ValidateProviderOutcome,
                "provider returned metadata for a different path",
            ));
        }
        Ok(response.into_metadata())
    }

    /// Returns `false` only for an explicit not-found response.
    pub fn exists(&self, path: &Path) -> FsResult<bool> {
        match self.stat(path) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == FsErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.with_operation(FsOperation::Exists)),
        }
    }

    /// Copies `source` to `target` through an optional provider fast path and a
    /// safe stream fallback.
    ///
    /// # Errors
    /// Returns [`CopyFailure`] with a recovery state. A provider failure is
    /// never retried; only an explicit provider decline can select the
    /// facade's allowlisted fallback.
    #[allow(clippy::result_large_err)]
    pub fn copy(&self, source: &Path, target: &Path, options: CopyOptions) -> Result<CopyOutcome, CopyFailure> {
        CopyOperation::new(self, source, target, options).execute()
    }

    /// Renames one resource through exactly one provider rename primitive.
    ///
    /// # Errors
    /// Returns [`RenameFailure`] with the provider-confirmed transition state.
    /// This method never implements rename through copy and delete.
    pub fn rename(&self, source: &Path, target: &Path, options: RenameOptions) -> Result<RenameOutcome, RenameFailure> {
        if let Err(error) = self.rename_preflight(source, target, &options) {
            return Err(self.contextualize_rename_failure(
                RenameFailure::new(error, RenameFailureState::Unchanged),
                source,
                target,
            ));
        }
        match self.spi.rename(RenameRequest::new(
            source,
            target,
            ResolvedRenameOptions::new(options.clone()),
        )) {
            Ok(outcome) => match validate_rename_outcome(&outcome, &options, source, target) {
                Some(violation) => Err(RenameFailure::new(
                    self.contract_error(source, FsOperation::Rename, violation.message)
                        .with_target(target.clone()),
                    violation.state,
                )),
                None => Ok(outcome),
            },
            Err(failure) => {
                let (error, state) = failure.into_parts();
                Err(RenameFailure::new(
                    error.with_operation(FsOperation::Rename).with_missing_context(
                        source,
                        Some(target),
                        self.properties().info().provider_id(),
                    ),
                    state,
                ))
            }
        }
    }

    /// Opens a provider directory stream after local option validation.
    pub fn list(&self, path: &Path, options: ListOptions) -> FsResult<DirectoryStream> {
        DirectoryOperation::new(self).list(path, options)
    }

    /// Opens a provider reader after local option validation.
    pub fn open_reader(&self, path: &Path, options: ReadOptions) -> FsResult<FileReader> {
        self.validate_path(path, FsOperation::OpenReader)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        self.properties()
            .limits()
            .validate_read_range(path, options.length())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        self.require(FileSystemCapability::Read, FsOperation::OpenReader, path)?;
        self.spi
            .open_reader(OpenReaderRequest::new(path, ResolvedReadOptions::new(options)))
            .and_then(|opened| {
                let (info, reader) = opened.into_parts();
                self.validate_opened_info(&info, path)?;
                Ok(FileReader::new(info, reader))
            })
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))
    }

    /// Opens a provider writer after local option validation.
    pub fn open_writer(&self, path: &Path, options: WriteOptions) -> FsResult<FileWriter> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenWriter))?;
        self.require(FileSystemCapability::Write, FsOperation::OpenWriter, path)?;
        let atomicity = options.atomicity();
        let durability = options.durability();
        self.spi
            .open_writer(OpenWriterRequest::new(path, ResolvedWriteOptions::new(options)))
            .and_then(|opened| {
                let (info, writer) = opened.into_parts();
                self.validate_opened_info(&info, path)?;
                Ok(FileWriter::new(
                    info,
                    writer,
                    atomicity,
                    durability,
                    self.properties().info().provider_id(),
                    self.properties().limits().max_write_bytes().maximum(),
                ))
            })
            .map_err(|error| self.enrich(error, path, FsOperation::OpenWriter))
    }

    /// Creates a temporary file and binds its provider session to this facade.
    pub fn create_temp_file(&self, options: TempOptions) -> FsResult<TempFile> {
        let parent = options.parent().cloned();
        self.core.validate_temp_parent(parent.as_ref())?;
        self.core
            .require(FileSystemCapability::TempFile, FsOperation::CreateTemp, parent.as_ref())?;
        self.spi
            .create_temp_file(CreateTempFileRequest::new(options))
            .and_then(|opened| {
                let (info, mut session) = opened.into_parts();
                if let Err(error) = self.validate_temp_info(&info, crate::metadata::FileKind::File) {
                    let path = error.path().cloned().unwrap_or_else(Path::root);
                    return Err(match session.cleanup() {
                        Ok(()) => error,
                        Err(cleanup) => FsError::with_source(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::ValidateProviderOutcome,
                            "provider returned an invalid temporary identity and cleanup failed",
                            cleanup,
                        )
                        .with_path(path.clone())
                        .with_provider(self.properties().info().provider_id()),
                    });
                }
                Ok(TempFile::new(self.clone(), info.path().clone(), session))
            })
            .map_err(|error| self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp))
    }

    /// Creates a temporary directory and binds its provider session to this
    /// facade.
    pub fn create_temp_directory(&self, options: TempOptions) -> FsResult<TempDirectory> {
        let parent = options.parent().cloned();
        self.core.validate_temp_parent(parent.as_ref())?;
        self.core.require(
            FileSystemCapability::TempDirectory,
            FsOperation::CreateTemp,
            parent.as_ref(),
        )?;
        self.spi
            .create_temp_directory(CreateTempDirectoryRequest::new(options))
            .and_then(|opened| {
                let (info, mut session) = opened.into_parts();
                if let Err(error) = self.validate_temp_info(&info, crate::metadata::FileKind::Directory) {
                    let path = error.path().cloned().unwrap_or_else(Path::root);
                    return Err(match session.cleanup() {
                        Ok(()) => error,
                        Err(cleanup) => FsError::with_source(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::ValidateProviderOutcome,
                            "provider returned an invalid temporary identity and cleanup failed",
                            cleanup,
                        )
                        .with_path(path.clone())
                        .with_provider(self.properties().info().provider_id()),
                    });
                }
                Ok(TempDirectory::new(self.clone(), info.path().clone(), session))
            })
            .map_err(|error| self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp))
    }

    /// Creates a directory after local path validation.
    pub fn create_directory(&self, path: &Path, options: CreateDirectoryOptions) -> FsResult<CreateDirectoryOutcome> {
        self.validate_path(path, FsOperation::CreateDir)?;
        self.require(FileSystemCapability::CreateDirectory, FsOperation::CreateDir, path)?;
        let exists_ok = options.exists_ok();
        let outcome = self
            .spi
            .create_directory(CreateDirectoryRequest::new(
                path,
                ResolvedCreateDirectoryOptions::new(options),
            ))
            .map_err(|error| self.enrich(error, path, FsOperation::CreateDir))?;
        if outcome.already_existed() && !exists_ok {
            return Err(self.contract_error(
                path,
                FsOperation::CreateDir,
                "provider accepted an existing directory without exists_ok",
            ));
        }
        Ok(outcome)
    }

    /// Deletes a file after local option validation.
    #[inline]
    pub fn delete_file(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false)
    }

    /// Deletes a directory after local option validation.
    #[inline]
    pub fn delete_directory(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, true)
    }

    /// Validates and dispatches one deletion primitive.
    fn delete(&self, path: &Path, options: DeleteOptions, directory: bool) -> FsResult<DeleteOutcome> {
        self.validate_path(path, FsOperation::Delete)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        self.require(FileSystemCapability::Delete, FsOperation::Delete, path)?;
        let missing_ok = options.missing_ok();
        let outcome = if directory {
            self.spi
                .delete_directory(DeleteDirectoryRequest::new(path, ResolvedDeleteOptions::new(options)))
        } else {
            self.spi
                .delete_file(DeleteFileRequest::new(path, ResolvedDeleteOptions::new(options)))
        }
        .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        if outcome.already_missing() && !missing_ok {
            return Err(self.contract_error(
                path,
                FsOperation::Delete,
                "provider accepted a missing target without missing_ok",
            ));
        }
        Ok(outcome)
    }

    /// Performs validation before the single rename provider call.
    fn rename_preflight(&self, source: &Path, target: &Path, options: &RenameOptions) -> FsResult<()> {
        self.validate_path(source, FsOperation::Rename)?;
        self.validate_path(target, FsOperation::Rename)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::Rename)
                    .with_target(target.clone())
            })?;
        self.require(FileSystemCapability::Rename, FsOperation::Rename, source)?;
        if source == target {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Rename,
                "rename source and target must differ",
            )
            .with_path(source.clone())
            .with_target(target.clone()));
        }
        Ok(())
    }

    /// Adds source, target, and provider facts required by public rename
    /// failures.
    fn contextualize_rename_failure(&self, failure: RenameFailure, source: &Path, target: &Path) -> RenameFailure {
        let (error, state) = failure.into_parts();
        RenameFailure::new(
            error.with_missing_context(source, Some(target), self.properties().info().provider_id()),
            state,
        )
    }

    /// Validates a path against the cached provider snapshot before I/O.
    fn validate_path(&self, path: &Path, operation: FsOperation) -> FsResult<()> {
        self.core.validate_path(path, operation)
    }

    /// Requires an advertised capability before I/O.
    fn require(&self, capability: FileSystemCapability, operation: FsOperation, path: &Path) -> FsResult<()> {
        self.core.require(capability, operation, Some(path))
    }

    /// Adds missing public error context to a provider error.
    fn enrich(&self, error: FsError, path: &Path, operation: FsOperation) -> FsError {
        self.core.enrich(error, Some(path), operation)
    }

    /// Builds a provider-contract error for an invalid outcome.
    fn contract_error(&self, path: &Path, operation: FsOperation, message: &str) -> FsError {
        self.core.contract_error(path, operation, message)
    }

    /// Validates the identity the provider attached to an opened file handle.
    fn validate_opened_info(&self, info: &crate::metadata::OpenedFileInfo, path: &Path) -> FsResult<()> {
        if info.filesystem_id() != self.properties().info().id() || info.path() != path {
            return Err(self.contract_error(
                path,
                FsOperation::ValidateProviderOutcome,
                "provider returned an opened handle with a different identity",
            ));
        }
        Ok(())
    }

    /// Validates the filesystem identity captured for a newly-created temporary
    /// resource.
    fn validate_temp_info(
        &self,
        info: &crate::metadata::OpenedFileInfo,
        expected_kind: crate::metadata::FileKind,
    ) -> FsResult<()> {
        if info.filesystem_id() != self.properties().info().id() {
            return Err(self.contract_error(
                info.path(),
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary handle for a different filesystem",
            ));
        }
        self.validate_path(info.path(), FsOperation::CreateTemp).map_err(|_| {
            self.contract_error(
                info.path(),
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary handle with an invalid logical path",
            )
        })?;
        if info.metadata().is_none_or(|metadata| metadata.kind() != &expected_kind) {
            return Err(self.contract_error(
                info.path(),
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary handle with an inconsistent resource kind",
            ));
        }
        Ok(())
    }

    /// Validates a temporary-resource persistence request before provider
    /// effects.
    pub(crate) fn preflight_temp_persist(
        &self,
        source: &Path,
        target: &Path,
        options: &crate::temp::PersistOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::PersistTemp)?;
        self.validate_path(target, FsOperation::PersistTemp)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::PersistTemp)
                    .with_target(target.clone())
            })?;
        Ok(())
    }

    /// Validates a provider-generated target reported by temporary keep.
    pub(crate) fn validate_temp_keep_target(&self, source: &Path, target: &Path) -> FsResult<()> {
        self.validate_path(target, FsOperation::KeepTemp).map_err(|_| {
            self.contract_error(
                source,
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary keep target with an invalid logical path",
            )
            .with_target(target.clone())
        })
    }

    /// Reads one file into memory up to `max_bytes` after opening a reader.
    pub fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        ReadOperation::new(self).read_all(path, options, max_bytes)
    }

    /// Reads at most max_bytes from a file without requiring a complete read.
    pub fn read_prefix(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        ReadOperation::new(self).read_prefix(path, options, max_bytes)
    }

    /// Writes all bytes and retains the writer if transfer or commit fails.
    pub fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<crate::metadata::WriteOutcome, WriteAllFailure> {
        WriteOperation::new(self).write_all(path, bytes, options)
    }
}
