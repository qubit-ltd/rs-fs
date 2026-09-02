// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem facade construction boundary.

use std::sync::Arc;

use crate::copy::AsyncCopyFailure;
use crate::copy::AsyncCopyOperation;
use crate::copy::CopyFailureState;
use crate::copy::CopyOptions;
use crate::copy::CopyOutcome;
use crate::copy::CopyStats;
use crate::directory::AsyncDirectoryOperation;
use crate::directory::AsyncDirectoryStream;
use crate::directory::CreateDirectoryOptions;
use crate::directory::CreateDirectoryOutcome;
use crate::directory::DeleteOptions;
use crate::directory::DeleteOutcome;
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
use crate::read::AsyncFileReader;
use crate::read::AsyncReadOperation;
use crate::read::ReadOptions;
use crate::rename::RenameFailure;
use crate::rename::RenameFailureState;
use crate::rename::RenameOptions;
use crate::rename::RenameOutcome;
use crate::rename::validate_rename_outcome;
use crate::spi::AsyncFileSystemSpi;
use crate::spi::CreateDirectoryRequest;
use crate::spi::DeleteDirectoryRequest;
use crate::spi::DeleteFileRequest;
use crate::spi::OpenReaderRequest;
use crate::spi::OpenWriterRequest;
use crate::spi::RenameRequest;
use crate::spi::ResolvedCreateDirectoryOptions;
use crate::spi::ResolvedDeleteOptions;
use crate::spi::ResolvedReadOptions;
use crate::spi::ResolvedRenameOptions;
use crate::spi::ResolvedWriteOptions;
use crate::spi::StatRequest;
use crate::temp::AsyncTempDirectory;
use crate::temp::AsyncTempFile;
use crate::temp::PersistOptions;
use crate::temp::TempOptions;
use crate::write::AsyncFileWriter;
use crate::write::AsyncWriteAllFailure;
use crate::write::AsyncWriteOperation;
use crate::write::WriteOptions;

/// Application-facing asynchronous filesystem facade.
///
/// It validates provider boundaries, exposes asynchronous operations, and owns
/// the cancellation-safe copy operation entry point.
#[derive(Clone)]
pub struct AsyncFileSystem {
    /// Provider implementation receiving validated asynchronous requests.
    spi: Arc<dyn AsyncFileSystemSpi>,
    /// Shared immutable state and deterministic preflight policy.
    core: Arc<FacadeCore>,
}

impl AsyncFileSystem {
    /// Constructs a facade and caches one validated provider snapshot.
    #[inline]
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: AsyncFileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    #[inline]
    pub fn from_shared_spi(spi: Arc<dyn AsyncFileSystemSpi>) -> FsResult<Self> {
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

    /// Returns the asynchronous provider implementation to operation objects.
    #[inline(always)]
    pub(crate) fn spi(&self) -> &dyn AsyncFileSystemSpi {
        self.spi.as_ref()
    }

    /// Asynchronously reads metadata after local validation completes.
    ///
    /// Local validation happens before this future reaches the provider. A
    /// provider response is checked again after awaiting to bind the returned
    /// metadata to the requested logical path.
    pub async fn stat(&self, path: &Path) -> FsResult<FileMetadata> {
        self.validate_path(path, FsOperation::Stat)?;
        let response = self
            .spi
            .stat(StatRequest::new(path, ()))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::Stat))?;
        if response.path() != path {
            return Err(self.contract_error(path, "provider returned metadata for a different path"));
        }
        Ok(response.into_metadata())
    }

    /// Asynchronously reports whether the path exists.
    pub async fn exists(&self, path: &Path) -> FsResult<bool> {
        match self.stat(path).await {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == FsErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.with_operation(FsOperation::Exists)),
        }
    }

    /// Asynchronously opens a validated directory stream.
    pub async fn list(&self, path: &Path, options: ListOptions) -> FsResult<AsyncDirectoryStream> {
        AsyncDirectoryOperation::new(self).list(path, options).await
    }

    /// Asynchronously opens a validated reader and verifies its identity.
    pub async fn open_reader(&self, path: &Path, options: ReadOptions) -> FsResult<AsyncFileReader> {
        self.validate_path(path, FsOperation::OpenReader)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        self.properties()
            .limits()
            .validate_read_range(path, options.length())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        self.require(FileSystemCapability::Read, FsOperation::OpenReader, path)?;
        let opened = self
            .spi
            .open_reader(OpenReaderRequest::new(path, ResolvedReadOptions::new(options)))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        self.validate_opened_info(opened.info(), path)?;
        Ok(opened.into_reader())
    }

    /// Asynchronously reads an entire file while enforcing a strict byte cap.
    pub async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        AsyncReadOperation::new(self).read_all(path, options, max_bytes).await
    }

    /// Asynchronously reads at most max_bytes from a file.
    pub async fn read_prefix(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        AsyncReadOperation::new(self)
            .read_prefix(path, options, max_bytes)
            .await
    }

    /// Asynchronously opens a validated writer and verifies its identity.
    pub async fn open_writer(&self, path: &Path, options: WriteOptions) -> FsResult<AsyncFileWriter> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::OpenWriter))?;
        self.require(FileSystemCapability::Write, FsOperation::OpenWriter, path)?;
        let atomicity = options.atomicity();
        let durability = options.durability();
        let opened = self
            .spi
            .open_writer(OpenWriterRequest::new(path, ResolvedWriteOptions::new(options)))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::OpenWriter))?;
        self.validate_opened_info(opened.info(), path)?;
        Ok(opened.into_writer(
            atomicity,
            durability,
            self.properties().info().provider_id(),
            self.properties().limits().max_write_bytes().maximum(),
        ))
    }

    /// Asynchronously writes all bytes and retains the writer on failure.
    pub async fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<crate::metadata::WriteOutcome, AsyncWriteAllFailure> {
        AsyncWriteOperation::new(self).write_all(path, bytes, options).await
    }

    /// Asynchronously creates a directory after local validation.
    pub async fn create_directory(
        &self,
        path: &Path,
        options: CreateDirectoryOptions,
    ) -> FsResult<CreateDirectoryOutcome> {
        self.validate_path(path, FsOperation::CreateDir)?;
        self.require(FileSystemCapability::CreateDirectory, FsOperation::CreateDir, path)?;
        let exists_ok = options.exists_ok();
        let outcome = self
            .spi
            .create_directory(CreateDirectoryRequest::new(
                path,
                ResolvedCreateDirectoryOptions::new(options),
            ))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::CreateDir))?;
        if outcome.already_existed() && !exists_ok {
            return Err(self.contract_error(path, "provider accepted an existing directory without exists_ok"));
        }
        Ok(outcome)
    }

    /// Asynchronously deletes a file after local validation.
    #[inline]
    pub async fn delete_file(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false).await
    }

    /// Asynchronously deletes a directory after local validation.
    #[inline]
    pub async fn delete_directory(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, true).await
    }

    /// Renames one resource through the single asynchronous provider primitive.
    pub async fn rename(
        &self,
        source: &Path,
        target: &Path,
        options: RenameOptions,
    ) -> Result<RenameOutcome, RenameFailure> {
        if let Err(error) = self.rename_preflight(source, target, &options) {
            return Err(self.contextual_rename_failure(error, RenameFailureState::Unchanged, source, target));
        }
        match self
            .spi
            .rename(RenameRequest::new(
                source,
                target,
                ResolvedRenameOptions::new(options.clone()),
            ))
            .await
        {
            Ok(outcome) => match validate_rename_outcome(&outcome, &options, source, target) {
                Some(violation) => Err(self.contextual_rename_failure(
                    self.contract_error(source, violation.message),
                    violation.state,
                    source,
                    target,
                )),
                None => Ok(outcome),
            },
            Err(failure) => {
                let (error, state) = failure.into_parts();
                Err(self.contextual_rename_failure(error, state, source, target))
            }
        }
    }

    /// Asynchronously creates a temporary file and validates its provider
    /// identity.
    pub async fn create_temp_file(&self, options: TempOptions) -> FsResult<AsyncTempFile> {
        let parent = options.parent().cloned();
        self.core.validate_temp_parent(parent.as_ref())?;
        self.core
            .require(FileSystemCapability::TempFile, FsOperation::CreateTemp, parent.as_ref())?;
        let opened = self
            .spi
            .create_temp_file(crate::spi::CreateTempFileRequest::new(options))
            .await
            .map_err(|error| self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp))?;
        let (info, session) = opened.into_parts();
        if let Err(error) = self.validate_temp_info(&info, crate::metadata::FileKind::File) {
            let path = error.path().cloned().unwrap_or_else(Path::root);
            let mut session = Box::into_pin(session);
            return Err(match session.as_mut().cleanup().await {
                Ok(()) => error,
                Err(cleanup) => FsError::with_source(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::ValidateProviderOutcome,
                    "provider returned an invalid temporary identity and cleanup failed",
                    cleanup,
                )
                .with_path(path)
                .with_provider(self.properties().info().provider_id()),
            });
        }
        Ok(AsyncTempFile::new(
            self.clone(),
            info.path().clone(),
            session,
            "temporary file",
        ))
    }

    /// Asynchronously creates a temporary directory and validates its identity.
    pub async fn create_temp_directory(&self, options: TempOptions) -> FsResult<AsyncTempDirectory> {
        let parent = options.parent().cloned();
        self.core.validate_temp_parent(parent.as_ref())?;
        self.core.require(
            FileSystemCapability::TempDirectory,
            FsOperation::CreateTemp,
            parent.as_ref(),
        )?;
        let opened = self
            .spi
            .create_temp_directory(crate::spi::CreateTempDirectoryRequest::new(options))
            .await
            .map_err(|error| self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp))?;
        let (info, session) = opened.into_parts();
        if let Err(error) = self.validate_temp_info(&info, crate::metadata::FileKind::Directory) {
            let mut session = Box::into_pin(session);
            return Err(match session.as_mut().cleanup().await {
                Ok(()) => error,
                Err(cleanup) => FsError::with_source(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::ValidateProviderOutcome,
                    "provider returned an invalid temporary identity and cleanup failed",
                    cleanup,
                )
                .with_path(error.path().cloned().unwrap_or_else(Path::root))
                .with_provider(self.properties().info().provider_id()),
            });
        }
        Ok(AsyncTempDirectory::new(self.clone(), info.path().clone(), session))
    }

    /// Begins a copy after synchronous path, option, and capability preflight.
    ///
    /// This method performs no provider I/O. Provider work begins only when
    /// [`AsyncCopyOperation::execute`] is polled.
    #[allow(clippy::result_large_err)]
    pub fn begin_copy(
        &self,
        source: Path,
        target: Path,
        options: CopyOptions,
    ) -> Result<AsyncCopyOperation, AsyncCopyFailure> {
        self.copy_preflight(&source, &target, &options).map_err(|error| {
            self.contextual_copy_failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                &source,
                &target,
            )
        })?;
        let symlink_policy = options
            .symlink_policy_override()
            .unwrap_or(self.properties().symlink_policy());
        Ok(AsyncCopyOperation::new(
            self.clone(),
            source,
            target,
            options,
            symlink_policy,
        ))
    }

    /// Rechecks provider-reported success against requested copy guarantees.
    #[allow(clippy::result_large_err)]
    pub(crate) fn verify_completed_copy(
        &self,
        outcome: CopyOutcome,
        options: &CopyOptions,
        source: &Path,
        target: &Path,
    ) -> Result<CopyOutcome, AsyncCopyFailure> {
        if let Some(message) = outcome.contract_violation(options) {
            return Err(self.contextual_copy_failure(
                FsError::new(FsErrorKind::ProviderContractViolation, FsOperation::Copy, message),
                CopyFailureState::Published,
                *outcome.stats(),
                source,
                target,
            ));
        }
        Ok(outcome)
    }

    /// Performs all no-I/O copy validation required before an operation exists.
    fn copy_preflight(&self, source: &Path, target: &Path, options: &CopyOptions) -> FsResult<()> {
        self.validate_path(source, FsOperation::Copy)?;
        self.validate_path(target, FsOperation::Copy)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::Copy)
                    .with_target(target.clone())
            })?;
        if source == target {
            return Err(FsError::new(
                crate::error::FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "copy source and target must differ",
            )
            .with_path(source.clone())
            .with_target(target.clone()));
        }
        Ok(())
    }

    /// Performs no-I/O validation for the single rename primitive.
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

    /// Dispatches the selected deletion primitive after local validation.
    async fn delete(&self, path: &Path, options: DeleteOptions, directory: bool) -> FsResult<DeleteOutcome> {
        self.validate_path(path, FsOperation::Delete)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        self.require(FileSystemCapability::Delete, FsOperation::Delete, path)?;
        let missing_ok = options.missing_ok();
        let request_options = ResolvedDeleteOptions::new(options);
        let outcome = if directory {
            self.spi
                .delete_directory(DeleteDirectoryRequest::new(path, request_options))
                .await
        } else {
            self.spi
                .delete_file(DeleteFileRequest::new(path, request_options))
                .await
        }
        .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        if outcome.already_missing() && !missing_ok {
            return Err(self.contract_error(path, "provider accepted a missing target without missing_ok"));
        }
        Ok(outcome)
    }

    /// Validates a logical path against the cached provider snapshot.
    fn validate_path(&self, path: &Path, operation: FsOperation) -> FsResult<()> {
        self.core.validate_path(path, operation)
    }

    /// Requires a provider-advertised capability before operation creation.
    pub(crate) fn require(
        &self,
        capability: FileSystemCapability,
        operation: FsOperation,
        path: &Path,
    ) -> FsResult<()> {
        self.core.require(capability, operation, Some(path))
    }

    /// Contextualizes a copy failure with its source, target, and provider
    /// facts.
    pub(crate) fn contextual_copy_failure(
        &self,
        error: FsError,
        state: CopyFailureState,
        stats: CopyStats,
        source: &Path,
        target: &Path,
    ) -> AsyncCopyFailure {
        AsyncCopyFailure::new(
            error.with_operation(FsOperation::Copy).with_missing_context(
                source,
                Some(target),
                self.properties().info().provider_id(),
            ),
            state,
            stats,
        )
    }

    /// Adds source, target, and provider facts to an asynchronous rename
    /// failure.
    fn contextual_rename_failure(
        &self,
        error: FsError,
        state: RenameFailureState,
        source: &Path,
        target: &Path,
    ) -> RenameFailure {
        RenameFailure::new(
            error.with_operation(FsOperation::Rename).with_missing_context(
                source,
                Some(target),
                self.properties().info().provider_id(),
            ),
            state,
        )
    }

    /// Adds missing public context to a provider error.
    fn enrich(&self, error: FsError, path: &Path, operation: FsOperation) -> FsError {
        self.core.enrich(error, Some(path), operation)
    }

    /// Creates a provider-contract violation bound to the requested path.
    fn contract_error(&self, path: &Path, message: &'static str) -> FsError {
        self.core
            .contract_error(path, FsOperation::ValidateProviderOutcome, message)
    }

    /// Adds a native read count to the public `u64` copy-statistics total.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::ResourceLimitExceeded`] when the native count or
    /// accumulated total cannot be represented by [`CopyStats`].
    pub(crate) fn add_copied_bytes(&self, total: u64, count: usize, source: &Path) -> FsResult<u64> {
        let count = u64::try_from(count).map_err(|_| self.copy_byte_count_error(source))?;
        total
            .checked_add(count)
            .ok_or_else(|| self.copy_byte_count_error(source))
    }

    /// Builds the copy-statistics error for an unrepresentable byte count.
    fn copy_byte_count_error(&self, source: &Path) -> FsError {
        FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Copy,
            "copy byte count exceeds the filesystem API reporting range",
        )
        .with_path(source.clone())
        .with_provider(self.properties().info().provider_id())
    }

    /// Validates a provider-opened handle identity before exposing it to
    /// callers.
    fn validate_opened_info(&self, info: &crate::metadata::OpenedFileInfo, path: &Path) -> FsResult<()> {
        if info.filesystem_id() != self.properties().info().id() || info.path() != path {
            return Err(self.contract_error(path, "provider returned an opened handle with a different identity"));
        }
        Ok(())
    }

    /// Validates a provider-created temporary resource before exposing it.
    fn validate_temp_info(
        &self,
        info: &crate::metadata::OpenedFileInfo,
        expected_kind: crate::metadata::FileKind,
    ) -> FsResult<()> {
        if info.filesystem_id() != self.properties().info().id() {
            return Err(self.contract_error(
                info.path(),
                "provider returned a temporary handle for a different filesystem",
            ));
        }
        self.validate_path(info.path(), FsOperation::CreateTemp).map_err(|_| {
            self.contract_error(
                info.path(),
                "provider returned a temporary handle with an invalid logical path",
            )
        })?;
        if info.metadata().is_none_or(|metadata| metadata.kind() != &expected_kind) {
            return Err(self.contract_error(
                info.path(),
                "provider returned a temporary handle with an inconsistent resource kind",
            ));
        }
        Ok(())
    }

    /// Performs no-I/O preflight for asynchronous temporary persistence.
    pub(crate) fn preflight_temp_persist(
        &self,
        source: &Path,
        target: &Path,
        options: &PersistOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::PersistTemp)?;
        self.validate_path(target, FsOperation::PersistTemp)?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::PersistTemp)
                    .with_target(target.clone())
            })
    }

    /// Validates a provider-generated target reported by temporary keep.
    pub(crate) fn validate_temp_keep_target(&self, source: &Path, target: &Path) -> FsResult<()> {
        self.validate_path(target, FsOperation::KeepTemp).map_err(|_| {
            self.contract_error(
                source,
                "provider returned a temporary keep target with an invalid logical path",
            )
            .with_target(target.clone())
        })
    }
}
