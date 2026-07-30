// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem facade construction boundary.

use std::sync::Arc;

use qubit_io::{
    AsyncInput,
    AsyncOutput,
};

use crate::spi::{
    AsyncFileSystemSpi,
    CopyAttempt,
    CopyRequest,
    CreateDirectoryRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    RenameRequest,
    ResolvedCopyOptions,
    ResolvedCreateDirectoryOptions,
    ResolvedDeleteOptions,
    ResolvedListOptions,
    ResolvedReadOptions,
    ResolvedRenameOptions,
    ResolvedWriteOptions,
    StatRequest,
};
use crate::{
    AsyncCopyFailure,
    AsyncCopyOperation,
    AsyncDirectoryStream,
    AsyncFileReader,
    AsyncFileWriter,
    AsyncTempDirectory,
    AsyncTempFile,
    CopyConflictPolicy,
    CopyFailureState,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirectoryOptions,
    CreateDirectoryOutcome,
    DeleteOptions,
    DeleteOutcome,
    FileMetadata,
    FileSystemCapability,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    ListOptions,
    Path,
    PersistOptions,
    ReadOptions,
    RenameFailure,
    RenameFailureState,
    RenameOptions,
    RenameOutcome,
    ServerSidePreference,
    TempDirectoryOptions,
    TempFileOptions,
    WriteDisposition,
    WriteOptions,
};

/// Application-facing asynchronous filesystem facade.
///
/// It validates provider boundaries, exposes asynchronous operations, and owns
/// the cancellation-safe copy operation entry point.
#[derive(Clone)]
pub struct AsyncFileSystem {
    spi: Arc<dyn AsyncFileSystemSpi>,
    properties: Arc<FileSystemProperties>,
}

impl AsyncFileSystem {
    /// Constructs a facade and caches one validated provider snapshot.
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: AsyncFileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    pub fn from_shared_spi(spi: Arc<dyn AsyncFileSystemSpi>) -> FsResult<Self> {
        let properties = spi.properties();
        properties.validate()?;
        Ok(Self {
            spi,
            properties: Arc::new(properties),
        })
    }

    /// Returns the immutable property snapshot without provider I/O.
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

    /// Asynchronously opens a validated directory stream.
    pub async fn list(
        &self,
        path: &Path,
        options: ListOptions,
    ) -> FsResult<AsyncDirectoryStream> {
        self.validate_path(path, FsOperation::List)?;
        self.require(FileSystemCapability::List, FsOperation::List, path)?;
        let options = ListOptions {
            page_size: self
                .properties
                .limits()
                .clamp_list_page_size(options.page_size),
            ..options
        };
        let opened = self
            .spi
            .list(ListRequest::new(
                path,
                ResolvedListOptions::new(options.clone()),
            ))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::List))?;
        Ok(opened.into_stream(
            path.clone(),
            options,
            self.properties.info().provider_id(),
        ))
    }

    /// Asynchronously opens a validated reader and verifies its identity.
    pub async fn open_reader(
        &self,
        path: &Path,
        options: ReadOptions,
    ) -> FsResult<AsyncFileReader> {
        self.validate_path(path, FsOperation::OpenReader)?;
        options.validate_against(self.properties.capabilities())?;
        self.properties
            .limits()
            .validate_read_range(path, options.length)?;
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

    /// Asynchronously opens a validated writer and verifies its identity.
    pub async fn open_writer(
        &self,
        path: &Path,
        options: WriteOptions,
    ) -> FsResult<AsyncFileWriter> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options.validate_against(self.properties.capabilities())?;
        self.require(
            FileSystemCapability::Write,
            FsOperation::OpenWriter,
            path,
        )?;
        let atomicity = options.atomicity;
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
        let exists_ok = options.exists_ok;
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
    pub async fn delete_file(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false).await
    }

    /// Asynchronously deletes a directory after local validation.
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
            Ok(outcome)
                if options.atomicity == crate::AtomicityRequirement::Required
                    && outcome.atomicity() != crate::AchievedAtomicity::Atomic =>
            {
                Err(self.contextual_rename_failure(
                    self.contract_error(
                        source,
                        "provider reported non-atomic success for an atomic-required rename",
                    ),
                    RenameFailureState::Renamed,
                    source,
                    target,
                ))
            }
            Ok(outcome) if outcome.method() == crate::PublicationMethod::CopyThenDelete => {
                Err(self.contextual_rename_failure(
                    self.contract_error(source, "provider returned copy-then-delete for rename"),
                    RenameFailureState::Renamed,
                    source,
                    target,
                ))
            }
            Ok(outcome) => Ok(outcome.with_identity(source, target)),
            Err(failure) => {
                let (error, state) = failure.into_parts();
                Err(self.contextual_rename_failure(error, state, source, target))
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
        if let Err(error) = self.validate_temp_info(&info) {
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
        Ok(AsyncTempFile::new(
            self.clone(),
            info.path().clone(),
            session,
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
        if let Err(error) = self.validate_temp_info(&info) {
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
        Ok(AsyncCopyOperation::new(
            self.clone(),
            source,
            target,
            options,
        ))
    }

    /// Runs the provider's asynchronous native-copy primitive.
    pub(crate) async fn execute_copy(
        &self,
        source: &Path,
        target: &Path,
        options: &ResolvedCopyOptions,
        writer: &mut Option<crate::AsyncFileWriter>,
    ) -> Result<CopyOutcome, AsyncCopyFailure> {
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
        if options.atomicity == crate::AtomicityRequirement::Required
            && outcome.atomicity() != crate::AchievedAtomicity::Atomic
        {
            return Err(self.contextual_copy_failure(
                FsError::new(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::Copy,
                    "provider reported non-atomic success for an atomic-required copy",
                ),
                CopyFailureState::Published,
                *outcome.stats(),
                source,
                target,
            ));
        }
        if options.durability == crate::DurabilityRequirement::Required
            && !outcome.durable()
        {
            return Err(self.contextual_copy_failure(
                FsError::new(
                    FsErrorKind::ProviderContractViolation,
                    FsOperation::Copy,
                    "provider reported non-durable success for a durability-required copy",
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
        options.validate_against(self.properties.capabilities())?;
        self.require(FileSystemCapability::Copy, FsOperation::Copy, source)?;
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
        options.validate_against(self.properties.capabilities())?;
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
        options.validate_against(self.properties.capabilities())?;
        self.require(FileSystemCapability::Delete, FsOperation::Delete, path)?;
        let missing_ok = options.missing_ok;
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
    async fn stream_copy_fallback(
        &self,
        source: &Path,
        target: &Path,
        options: &ResolvedCopyOptions,
        writer_slot: &mut Option<crate::AsyncFileWriter>,
    ) -> Result<CopyOutcome, AsyncCopyFailure> {
        let options = options.options();
        if options.continue_on_error
            || options.preserve_metadata != crate::MetadataPreservePolicy::None
            || options.server_side == ServerSidePreference::Require
            || options.create_parent
            || options.durability == crate::DurabilityRequirement::Required
            || (options.conflict == CopyConflictPolicy::Skip
                && options.atomicity == crate::AtomicityRequirement::Required)
            || !matches!(
                options.conflict,
                CopyConflictPolicy::Fail | CopyConflictPolicy::Skip
            )
        {
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
        if !matches!(
            metadata.kind,
            crate::FileKind::File | crate::FileKind::Object
        ) {
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
        if let Some(length) = metadata.len {
            if let Err(error) = self
                .properties
                .limits()
                .validate_read_range(source, Some(length))
            {
                return Err(self.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
            let length = match usize::try_from(length) {
                Ok(length) => length,
                Err(_) => {
                    return Err(self.contextual_copy_failure(
                        FsError::new(
                            FsErrorKind::ResourceLimitExceeded,
                            FsOperation::Copy,
                            "source length cannot fit in a write session",
                        ),
                        CopyFailureState::Unchanged,
                        CopyStats::default(),
                        source,
                        target,
                    ));
                }
            };
            if let Err(error) =
                self.properties.limits().validate_write_size(target, length)
            {
                return Err(self.contextual_copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    source,
                    target,
                ));
            }
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
        let writer_options = WriteOptions {
            disposition: WriteDisposition::CreateNew,
            atomicity: options.atomicity,
            ..WriteOptions::default()
        };
        match self.open_writer(target, writer_options).await {
            Ok(writer) => *writer_slot = Some(writer),
            Err(error)
                if error.kind() == FsErrorKind::AlreadyExists
                    && options.conflict == CopyConflictPolicy::Skip =>
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
                        CopyFailureState::PartiallyPublished,
                        fallback_failure_stats(bytes),
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
                        CopyFailureState::PartiallyPublished,
                        fallback_failure_stats(bytes),
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
                CopyFailureState::PartiallyPublished,
                fallback_failure_stats(bytes),
                source,
                target,
            )
        })?;
        let writer = writer_slot
            .as_mut()
            .expect("writer is retained before commit");
        let write_outcome = writer.commit_async().await.map_err(|error| {
            let state = match writer.state() {
                crate::WriterState::Published => CopyFailureState::Published,
                crate::WriterState::Indeterminate => {
                    CopyFailureState::Indeterminate
                }
                _ => CopyFailureState::PartiallyPublished,
            };
            self.contextual_copy_failure(
                error,
                state,
                fallback_failure_stats(bytes),
                source,
                target,
            )
        })?;
        let _ = writer_slot.take();
        Ok(CopyOutcome::streamed_fallback(
            CopyStats {
                files: 1,
                bytes,
                ..CopyStats::default()
            },
            write_outcome.atomicity,
        ))
    }

    /// Validates a logical path against the cached provider snapshot.
    fn validate_path(
        &self,
        path: &Path,
        operation: FsOperation,
    ) -> FsResult<()> {
        if path.semantics() != self.properties.info().path_semantics() {
            return Err(FsError::invalid_path(
                operation,
                "path semantics do not match this filesystem",
            ));
        }
        self.properties.path_constraints().validate(path)?;
        self.properties.limits().validate_path(
            path,
            self.properties.info().path_semantics(),
            operation,
        )
    }

    /// Requires a provider-advertised capability before operation creation.
    fn require(
        &self,
        capability: FileSystemCapability,
        operation: FsOperation,
        path: &Path,
    ) -> FsResult<()> {
        if self.properties.capabilities().contains(capability) {
            Ok(())
        } else {
            Err(FsError::new(
                crate::FsErrorKind::UnsupportedCapability,
                operation,
                "filesystem capability is not supported",
            )
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id())
            .with_required_capability(capability))
        }
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
        error.with_operation(operation).with_missing_context(
            path,
            None,
            self.properties.info().provider_id(),
        )
    }

    /// Creates a provider-contract violation bound to the requested path.
    fn contract_error(&self, path: &Path, message: &'static str) -> FsError {
        FsError::new(
            FsErrorKind::ProviderContractViolation,
            FsOperation::ValidateProviderOutcome,
            message,
        )
        .with_path(path.clone())
        .with_provider(self.properties.info().provider_id())
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
    fn validate_temp_info(&self, info: &crate::OpenedFileInfo) -> FsResult<()> {
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
            })
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
        options.validate_against(self.properties.capabilities())
    }
}

/// Builds partial statistics for a failed streamed copy.
fn fallback_failure_stats(bytes: u64) -> CopyStats {
    CopyStats {
        files: 1,
        bytes,
        failed: 1,
        ..CopyStats::default()
    }
}
