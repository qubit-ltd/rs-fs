// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem facade construction boundary.

use std::sync::Arc;

use qubit_io::AsyncInput;
use qubit_io::AsyncOutput;

use crate::AsyncCopyFailure;
use crate::AsyncCopyOperation;
use crate::AsyncDirectoryStream;
use crate::AsyncFileReader;
use crate::AsyncFileWriter;
use crate::AsyncTempDirectory;
use crate::AsyncTempFile;
use crate::AsyncWriteAllFailure;
use crate::CopyConflictPolicy;
use crate::CopyFailureState;
use crate::CopyOptions;
use crate::CopyOutcome;
use crate::CopyStats;
use crate::CreateDirectoryOptions;
use crate::CreateDirectoryOutcome;
use crate::DeleteOptions;
use crate::DeleteOutcome;
use crate::FileMetadata;
use crate::FileSystemCapability;
use crate::FileSystemProperties;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::FsResult;
use crate::ListOptions;
use crate::Path;
use crate::PersistOptions;
use crate::ReadOptions;
use crate::RenameFailure;
use crate::RenameFailureState;
use crate::RenameOptions;
use crate::RenameOutcome;
use crate::TempDirectoryOptions;
use crate::TempFileOptions;
use crate::WriteDisposition;
use crate::WriteOptions;
use crate::copy::fallback_failure_stats;
use crate::copy::fallback_options_supported;
use crate::copy::from_writer_state;
use crate::copy::is_file_kind_supported;
use crate::copy::validate_stream_copy_length_limits;
use crate::internal::facade::read_policy::PREFIX_BUFFER_SIZE;
use crate::internal::facade::read_policy::next_read_len;
use crate::rename::validate_rename_outcome;
use crate::spi::AsyncFileSystemSpi;
use crate::spi::CopyAttempt;
use crate::spi::CopyRequest;
use crate::spi::CreateDirectoryRequest;
use crate::spi::DeleteDirectoryRequest;
use crate::spi::DeleteFileRequest;
use crate::spi::ListRequest;
use crate::spi::OpenReaderRequest;
use crate::spi::OpenWriterRequest;
use crate::spi::RenameRequest;
use crate::spi::ResolvedCopyOptions;
use crate::spi::ResolvedCreateDirectoryOptions;
use crate::spi::ResolvedDeleteOptions;
use crate::spi::ResolvedListOptions;
use crate::spi::ResolvedReadOptions;
use crate::spi::ResolvedRenameOptions;
use crate::spi::ResolvedWriteOptions;
use crate::spi::SpiFuture;
use crate::spi::StatRequest;

/// Application-facing asynchronous filesystem facade.
///
/// It validates provider boundaries, exposes asynchronous operations, and owns
/// the cancellation-safe copy operation entry point.
#[derive(Clone)]
pub struct AsyncFileSystem {
    /// Provider implementation receiving validated asynchronous requests.
    spi: Arc<dyn AsyncFileSystemSpi>,
    /// Immutable provider properties captured when the facade was created.
    properties: Arc<FileSystemProperties>,
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
        let properties = spi.properties();
        properties.validate()?;
        Ok(Self {
            spi,
            properties: Arc::new(properties),
        })
    }

    /// Returns the immutable property snapshot without provider I/O.
    #[inline(always)]
    #[must_use]
    pub fn properties(&self) -> &FileSystemProperties {
        &self.properties
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
            return Err(self.contract_error(
                path,
                "provider returned metadata for a different path",
            ));
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
    pub async fn list(
        &self,
        path: &Path,
        options: ListOptions,
    ) -> FsResult<AsyncDirectoryStream> {
        self.validate_path(path, FsOperation::List)?;
        options
            .validate()
            .map_err(|error| self.enrich(error, path, FsOperation::List))?;
        self.require(FileSystemCapability::List, FsOperation::List, path)?;
        let page_size = self
            .properties
            .limits()
            .clamp_list_page_size(options.page_size());
        let options = options.with_page_size(page_size);
        let opened = self
            .spi
            .list(ListRequest::new(
                path,
                ResolvedListOptions::new(
                    options.clone(),
                    options
                        .symlink_policy_override()
                        .unwrap_or(self.properties.symlink_policy()),
                ),
            ))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::List))?;
        Ok(opened.into_stream(
            path.clone(),
            options,
            self.properties.info().provider_id(),
            self.properties.info().path_semantics(),
            *self.properties.limits(),
        ))
    }

    /// Asynchronously opens a validated reader and verifies its identity.
    pub async fn open_reader(
        &self,
        path: &Path,
        options: ReadOptions,
    ) -> FsResult<AsyncFileReader> {
        self.validate_path(path, FsOperation::OpenReader)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, path, FsOperation::OpenReader)
            })?;
        self.properties
            .limits()
            .validate_read_range(path, options.length())
            .map_err(|error| {
                self.enrich(error, path, FsOperation::OpenReader)
            })?;
        self.require(
            FileSystemCapability::Read,
            FsOperation::OpenReader,
            path,
        )?;
        let opened = self
            .spi
            .open_reader(OpenReaderRequest::new(
                path,
                ResolvedReadOptions::new(options),
            ))
            .await
            .map_err(|error| {
                self.enrich(error, path, FsOperation::OpenReader)
            })?;
        self.validate_opened_info(opened.info(), path)?;
        Ok(opened.into_reader())
    }

    /// Asynchronously reads an entire file while enforcing a strict byte cap.
    pub async fn read_all(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<Vec<u8>> {
        let mut reader = self.open_reader(path, options).await?;
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let remaining = max_bytes.saturating_sub(bytes.len());
            let read_len = remaining.saturating_add(1).min(buffer.len());
            let read = reader
                .read_async(&mut buffer[..read_len])
                .await
                .map_err(|error| {
                    self.enrich(
                        FsError::from_stream_io(error, FsOperation::Read, path),
                        path,
                        FsOperation::Read,
                    )
                })?;
            if read == 0 {
                return Ok(bytes);
            }
            if read > remaining {
                return Err(FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Read,
                    "file exceeds the configured read limit",
                )
                .with_path(path.clone())
                .with_provider(self.properties.info().provider_id()));
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
    }

    /// Asynchronously reads at most max_bytes from a file.
    pub async fn read_prefix(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<Vec<u8>> {
        let mut reader = self.open_reader(path, options).await?;
        if max_bytes == 0 {
            return Ok(Vec::new());
        }
        let mut bytes = Vec::with_capacity(max_bytes.min(PREFIX_BUFFER_SIZE));
        let mut buffer = [0_u8; PREFIX_BUFFER_SIZE];
        while bytes.len() < max_bytes {
            let read_len = next_read_len(bytes.len(), max_bytes);
            let read = reader
                .read_async(&mut buffer[..read_len])
                .await
                .map_err(|error| {
                    self.enrich(
                        FsError::from_stream_io(error, FsOperation::Read, path),
                        path,
                        FsOperation::Read,
                    )
                })?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
        }
        Ok(bytes)
    }

    /// Asynchronously opens a validated writer and verifies its identity.
    pub async fn open_writer(
        &self,
        path: &Path,
        options: WriteOptions,
    ) -> FsResult<AsyncFileWriter> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, path, FsOperation::OpenWriter)
            })?;
        self.require(
            FileSystemCapability::Write,
            FsOperation::OpenWriter,
            path,
        )?;
        let atomicity = options.atomicity();
        let opened = self
            .spi
            .open_writer(OpenWriterRequest::new(
                path,
                ResolvedWriteOptions::new(options),
            ))
            .await
            .map_err(|error| {
                self.enrich(error, path, FsOperation::OpenWriter)
            })?;
        self.validate_opened_info(opened.info(), path)?;
        Ok(opened.into_writer(
            atomicity,
            self.properties.info().provider_id(),
            self.properties.limits().max_write_bytes().maximum(),
        ))
    }

    /// Asynchronously writes all bytes and retains the writer on failure.
    pub async fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<crate::WriteOutcome, AsyncWriteAllFailure> {
        if let Err(error) = self
            .properties
            .limits()
            .validate_write_size(path, bytes.len())
        {
            return Err(AsyncWriteAllFailure::new(
                self.enrich(error, path, FsOperation::Write),
                None,
            ));
        }
        let mut writer = self
            .open_writer(path, options)
            .await
            .map_err(|error| AsyncWriteAllFailure::new(error, None))?;
        if let Err(error) = writer.write_fully_async(bytes).await {
            return Err(AsyncWriteAllFailure::new(
                self.enrich(
                    FsError::from_stream_io(error, FsOperation::Write, path),
                    path,
                    FsOperation::Write,
                ),
                Some(writer),
            ));
        }
        if let Err(error) = writer.flush_async().await {
            return Err(AsyncWriteAllFailure::new(
                self.enrich(
                    FsError::from_stream_io(error, FsOperation::Write, path),
                    path,
                    FsOperation::Write,
                ),
                Some(writer),
            ));
        }
        writer.commit_async().await.map_err(|failure| {
            AsyncWriteAllFailure::new(failure.into_error(), Some(writer))
        })
    }

    /// Asynchronously creates a directory after local validation.
    pub async fn create_directory(
        &self,
        path: &Path,
        options: CreateDirectoryOptions,
    ) -> FsResult<CreateDirectoryOutcome> {
        self.validate_path(path, FsOperation::CreateDir)?;
        self.require(
            FileSystemCapability::CreateDirectory,
            FsOperation::CreateDir,
            path,
        )?;
        let exists_ok = options.exists_ok();
        let outcome = self
            .spi
            .create_directory(CreateDirectoryRequest::new(
                path,
                ResolvedCreateDirectoryOptions::new(options),
            ))
            .await
            .map_err(|error| {
                self.enrich(error, path, FsOperation::CreateDir)
            })?;
        if outcome.already_existed() && !exists_ok {
            return Err(self.contract_error(
                path,
                "provider accepted an existing directory without exists_ok",
            ));
        }
        Ok(outcome)
    }

    /// Asynchronously deletes a file after local validation.
    #[inline]
    pub async fn delete_file(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false).await
    }

    /// Asynchronously deletes a directory after local validation.
    #[inline]
    pub async fn delete_directory(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> FsResult<DeleteOutcome> {
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
            return Err(self.contextual_rename_failure(
                error,
                RenameFailureState::Unchanged,
                source,
                target,
            ));
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
            Ok(outcome) => match validate_rename_outcome(
                &outcome, &options, source, target,
            ) {
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
                Err(self
                    .contextual_rename_failure(error, state, source, target))
            }
        }
    }

    /// Asynchronously creates a temporary file and validates its provider
    /// identity.
    pub async fn create_temp_file(
        &self,
        options: TempFileOptions,
    ) -> FsResult<AsyncTempFile> {
        self.require(
            FileSystemCapability::TempFile,
            FsOperation::CreateTemp,
            &Path::root(),
        )?;
        let opened = self
            .spi
            .create_temp_file(crate::spi::CreateTempFileRequest::new(options))
            .await
            .map_err(|error| {
                self.enrich(error, &Path::root(), FsOperation::CreateTemp)
            })?;
        let (info, session) = opened.into_parts();
        if let Err(error) =
            self.validate_temp_info(&info, crate::FileKind::File)
        {
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
                .with_provider(self.properties.info().provider_id()),
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
    pub async fn create_temp_directory(
        &self,
        options: TempDirectoryOptions,
    ) -> FsResult<AsyncTempDirectory> {
        self.require(
            FileSystemCapability::TempDirectory,
            FsOperation::CreateTemp,
            &Path::root(),
        )?;
        let opened = self
            .spi
            .create_temp_directory(crate::spi::CreateTempDirectoryRequest::new(
                options,
            ))
            .await
            .map_err(|error| {
                self.enrich(error, &Path::root(), FsOperation::CreateTemp)
            })?;
        let (info, session) = opened.into_parts();
        if let Err(error) =
            self.validate_temp_info(&info, crate::FileKind::Directory)
        {
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
                .with_provider(self.properties.info().provider_id()),
            });
        }
        Ok(AsyncTempDirectory::new(
            self.clone(),
            info.path().clone(),
            session,
        ))
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
        self.copy_preflight(&source, &target, &options)
            .map_err(|error| {
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
            .unwrap_or(self.properties.symlink_policy());
        Ok(AsyncCopyOperation::new(
            self.clone(),
            source,
            target,
            options,
            symlink_policy,
        ))
    }

    /// Runs the provider's asynchronous native-copy primitive.
    pub(crate) async fn execute_copy(
        &self,
        source: &Path,
        target: &Path,
        options: &ResolvedCopyOptions,
        writer: &mut Option<Box<crate::AsyncFileWriter>>,
    ) -> Result<CopyOutcome, AsyncCopyFailure> {
        if !self
            .properties
            .capabilities()
            .supports(FileSystemCapability::Copy)
        {
            return self
                .stream_copy_fallback(source, target, options, writer)
                .await;
        }
        match self
            .spi
            .try_copy(CopyRequest::new(source, target, options.clone()))
            .await
        {
            Ok(CopyAttempt::Completed(outcome)) => self.verify_completed_copy(
                outcome,
                options.options(),
                source,
                target,
            ),
            Ok(CopyAttempt::Declined(_)) => {
                self.stream_copy_fallback(source, target, options, writer)
                    .await
            }
            Err(failure) => {
                let (error, state, stats) = failure.into_parts();
                Err(self.contextual_copy_failure(
                    error, state, stats, source, target,
                ))
            }
        }
    }

    /// Rechecks provider-reported success against requested copy guarantees.
    #[allow(clippy::result_large_err)]
    fn verify_completed_copy(
        &self,
        outcome: CopyOutcome,
        options: &CopyOptions,
        source: &Path,
        target: &Path,
    ) -> Result<CopyOutcome, AsyncCopyFailure> {
        if let Some(message) = outcome.contract_violation(options) {
            return Err(self.contextual_copy_failure(
                FsError::new(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::Copy,
                    message,
                ),
                CopyFailureState::Published,
                *outcome.stats(),
                source,
                target,
            ));
        }
        Ok(outcome)
    }

    /// Performs all no-I/O copy validation required before an operation exists.
    fn copy_preflight(
        &self,
        source: &Path,
        target: &Path,
        options: &CopyOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::Copy)?;
        self.validate_path(target, FsOperation::Copy)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::Copy)
                    .with_target(target.clone())
            })?;
        if source == target {
            return Err(FsError::new(
                crate::FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "copy source and target must differ",
            )
            .with_path(source.clone())
            .with_target(target.clone()));
        }
        Ok(())
    }

    /// Performs no-I/O validation for the single rename primitive.
    fn rename_preflight(
        &self,
        source: &Path,
        target: &Path,
        options: &RenameOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::Rename)?;
        self.validate_path(target, FsOperation::Rename)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::Rename)
                    .with_target(target.clone())
            })?;
        self.require(
            FileSystemCapability::Rename,
            FsOperation::Rename,
            source,
        )?;
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
    async fn delete(
        &self,
        path: &Path,
        options: DeleteOptions,
        directory: bool,
    ) -> FsResult<DeleteOutcome> {
        self.validate_path(path, FsOperation::Delete)?;
        options
            .validate_against(self.properties.capabilities())
            .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        self.require(FileSystemCapability::Delete, FsOperation::Delete, path)?;
        let missing_ok = options.missing_ok();
        let request_options = ResolvedDeleteOptions::new(options);
        let outcome = if directory {
            self.spi
                .delete_directory(DeleteDirectoryRequest::new(
                    path,
                    request_options,
                ))
                .await
        } else {
            self.spi
                .delete_file(DeleteFileRequest::new(path, request_options))
                .await
        }
        .map_err(|error| self.enrich(error, path, FsOperation::Delete))?;
        if outcome.already_missing() && !missing_ok {
            return Err(self.contract_error(
                path,
                "provider accepted a missing target without missing_ok",
            ));
        }
        Ok(outcome)
    }

    /// Streams a declined native copy through facade-owned asynchronous
    /// handles.
    fn stream_copy_fallback<'a>(
        &'a self,
        source: &'a Path,
        target: &'a Path,
        options: &'a ResolvedCopyOptions,
        writer_slot: &'a mut Option<Box<crate::AsyncFileWriter>>,
    ) -> SpiFuture<'a, Result<CopyOutcome, AsyncCopyFailure>> {
        Box::pin(async move {
            let options = options.options();
            if !fallback_options_supported(options) {
                return Err(self.contextual_copy_failure(
                    FsError::new(
                        FsErrorKind::RequirementNotMet,
                        FsOperation::Copy,
                        "declined copy cannot use the stream fallback for these options",
                    ),
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
            self.require(FileSystemCapability::Read, FsOperation::Copy, source)
                .and_then(|_| {
                    self.require(
                        FileSystemCapability::Write,
                        FsOperation::Copy,
                        target,
                    )
                })
                .map_err(|error| {
                    self.contextual_copy_failure(
                        error,
                        CopyFailureState::Unchanged,
                        CopyStats::default(),
                        source,
                        target,
                    )
                })?;
            let metadata = self.stat(source).await.map_err(|error| {
                self.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                )
            })?;
            if !is_file_kind_supported(metadata.kind().clone()) {
                return Err(self.contextual_copy_failure(
                    FsError::new(
                        FsErrorKind::InvalidOptions,
                        FsOperation::Copy,
                        "stream fallback only supports regular files and objects",
                    ),
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
            if let Some(length) = metadata.len()
                && let Err(error) = validate_stream_copy_length_limits(
                    self.properties.limits(),
                    source,
                    target,
                    length,
                )
            {
                return Err(self.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
            let mut reader = self
                .open_reader(source, ReadOptions::default())
                .await
                .map_err(|error| {
                    self.contextual_copy_failure(
                        error,
                        CopyFailureState::Unchanged,
                        CopyStats::default(),
                        source,
                        target,
                    )
                })?;
            let writer_options = WriteOptions::default()
                .with_disposition(WriteDisposition::CreateNew)
                .with_atomicity(options.atomicity());
            match self.open_writer(target, writer_options).await {
                Ok(writer) => *writer_slot = Some(Box::new(writer)),
                Err(error)
                    if error.kind() == FsErrorKind::AlreadyExists
                        && options.conflict() == CopyConflictPolicy::Skip =>
                {
                    return Ok(CopyOutcome::streamed_fallback(
                        CopyStats {
                            skipped: 1,
                            ..CopyStats::default()
                        },
                        crate::AchievedAtomicity::NonAtomic,
                    ));
                }
                Err(error) => {
                    return Err(self.contextual_copy_failure(
                        error,
                        CopyFailureState::Unchanged,
                        CopyStats::default(),
                        source,
                        target,
                    ));
                }
            }
            let mut bytes = 0_u64;
            let mut buffer = [0_u8; 8192];
            loop {
                let read =
                    reader.read_async(&mut buffer).await.map_err(|error| {
                        self.contextual_copy_failure(
                            FsError::from_stream_io(
                                error,
                                FsOperation::Read,
                                source,
                            ),
                            from_writer_state(
                                writer_slot
                                    .as_ref()
                                    .expect(
                                        "writer is retained before transfer",
                                    )
                                    .state(),
                            ),
                            fallback_failure_stats(
                                writer_slot
                                    .as_ref()
                                    .expect(
                                        "writer is retained before transfer",
                                    )
                                    .written_bytes(),
                            ),
                            source,
                            target,
                        )
                    })?;
                if read == 0 {
                    break;
                }
                let writer = writer_slot
                    .as_mut()
                    .expect("writer is retained before transfer");
                writer.write_fully_async(&buffer[..read]).await.map_err(
                    |error| {
                        self.contextual_copy_failure(
                            FsError::from_stream_io(
                                error,
                                FsOperation::Write,
                                target,
                            ),
                            from_writer_state(writer.state()),
                            fallback_failure_stats(writer.written_bytes()),
                            source,
                            target,
                        )
                    },
                )?;
                bytes = bytes.saturating_add(read as u64);
            }
            let writer = writer_slot
                .as_mut()
                .expect("writer is retained before flush");
            writer.flush_async().await.map_err(|error| {
                self.contextual_copy_failure(
                    FsError::from_stream_io(error, FsOperation::Write, target),
                    from_writer_state(writer.state()),
                    fallback_failure_stats(writer.written_bytes()),
                    source,
                    target,
                )
            })?;
            let writer = writer_slot
                .as_mut()
                .expect("writer is retained before commit");
            let write_outcome = match writer.commit_async().await {
                Ok(outcome) => outcome,
                Err(failure)
                    if failure.error().kind() == FsErrorKind::AlreadyExists
                        && options.conflict() == CopyConflictPolicy::Skip
                        && from_writer_state(writer.state())
                            == CopyFailureState::Unchanged =>
                {
                    if let Err(cleanup_error) = writer.abort_async().await {
                        return Err(self.contextual_copy_failure(
                            cleanup_error,
                            from_writer_state(writer.state()),
                            fallback_failure_stats(writer.written_bytes()),
                            source,
                            target,
                        ));
                    }
                    let _ = writer_slot.take();
                    return Ok(CopyOutcome::streamed_fallback(
                        CopyStats {
                            skipped: 1,
                            ..CopyStats::default()
                        },
                        crate::AchievedAtomicity::NonAtomic,
                    ));
                }
                Err(failure) => {
                    return Err(self.contextual_copy_failure(
                        failure.into_error(),
                        from_writer_state(writer.state()),
                        fallback_failure_stats(writer.written_bytes()),
                        source,
                        target,
                    ));
                }
            };
            let _ = writer_slot.take();
            Ok(CopyOutcome::streamed_fallback(
                CopyStats {
                    files: 1,
                    bytes,
                    ..CopyStats::default()
                },
                write_outcome.atomicity(),
            ))
        })
    }

    /// Validates a logical path against the cached provider snapshot.
    fn validate_path(
        &self,
        path: &Path,
        operation: FsOperation,
    ) -> FsResult<()> {
        crate::facade_context::validate_path(&self.properties, path, operation)
    }

    /// Requires a provider-advertised capability before operation creation.
    fn require(
        &self,
        capability: FileSystemCapability,
        operation: FsOperation,
        path: &Path,
    ) -> FsResult<()> {
        crate::facade_context::require(
            &self.properties,
            capability,
            operation,
            path,
        )
    }

    /// Contextualizes a copy failure with its source, target, and provider
    /// facts.
    fn contextual_copy_failure(
        &self,
        error: FsError,
        state: CopyFailureState,
        stats: CopyStats,
        source: &Path,
        target: &Path,
    ) -> AsyncCopyFailure {
        AsyncCopyFailure::new(
            error
                .with_operation(FsOperation::Copy)
                .with_missing_context(
                    source,
                    Some(target),
                    self.properties.info().provider_id(),
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
            error
                .with_operation(FsOperation::Rename)
                .with_missing_context(
                    source,
                    Some(target),
                    self.properties.info().provider_id(),
                ),
            state,
        )
    }

    /// Adds missing public context to a provider error.
    fn enrich(
        &self,
        error: FsError,
        path: &Path,
        operation: FsOperation,
    ) -> FsError {
        crate::facade_context::enrich(&self.properties, error, path, operation)
    }

    /// Creates a provider-contract violation bound to the requested path.
    fn contract_error(&self, path: &Path, message: &'static str) -> FsError {
        crate::facade_context::contract_error(
            &self.properties,
            path,
            FsOperation::ValidateProviderOutcome,
            message,
        )
    }

    /// Validates a provider-opened handle identity before exposing it to
    /// callers.
    fn validate_opened_info(
        &self,
        info: &crate::OpenedFileInfo,
        path: &Path,
    ) -> FsResult<()> {
        if info.filesystem_id() != self.properties.info().id()
            || info.path() != path
        {
            return Err(self.contract_error(
                path,
                "provider returned an opened handle with a different identity",
            ));
        }
        Ok(())
    }

    /// Validates a provider-created temporary resource before exposing it.
    fn validate_temp_info(
        &self,
        info: &crate::OpenedFileInfo,
        expected_kind: crate::FileKind,
    ) -> FsResult<()> {
        if info.filesystem_id() != self.properties.info().id() {
            return Err(self.contract_error(
                info.path(),
                "provider returned a temporary handle for a different filesystem",
            ));
        }
        self.validate_path(info.path(), FsOperation::CreateTemp)
            .map_err(|_| {
                self.contract_error(
                    info.path(),
                    "provider returned a temporary handle with an invalid logical path",
                )
            })?;
        if info
            .metadata()
            .is_none_or(|metadata| metadata.kind() != &expected_kind)
        {
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
            .validate_against(self.properties.capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::PersistTemp)
                    .with_target(target.clone())
            })
    }
}
