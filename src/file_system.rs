// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validating synchronous filesystem facade.

use std::io::Error as IoError;
use std::sync::Arc;

use qubit_io::{
    Input,
    Output,
};

use crate::spi::{
    CopyAttempt,
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    FileSystemSpi,
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
    CopyConflictPolicy,
    CopyFailure,
    CopyFailureState,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirectoryOptions,
    CreateDirectoryOutcome,
    DeleteOptions,
    DeleteOutcome,
    DirectoryStream,
    FileKind,
    FileMetadata,
    FileReader,
    FileSystemCapability,
    FileSystemProperties,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    ListOptions,
    Path,
    ReadOptions,
    RenameFailure,
    RenameFailureState,
    RenameOptions,
    RenameOutcome,
    ServerSidePreference,
    TempDirectory,
    TempDirectoryOptions,
    TempFile,
    TempFileOptions,
    WriteAllFailure,
    WriteDisposition,
    WriteFailureState,
    WriteOptions,
};

/// Application-facing synchronous filesystem facade.
#[derive(Clone)]
pub struct FileSystem {
    spi: Arc<dyn FileSystemSpi>,
    properties: Arc<FileSystemProperties>,
}

impl FileSystem {
    /// Constructs a facade and caches the provider's single validated property
    /// snapshot.
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: FileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    pub fn from_shared_spi(spi: Arc<dyn FileSystemSpi>) -> FsResult<Self> {
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
    pub fn copy(
        &self,
        source: &Path,
        target: &Path,
        options: CopyOptions,
    ) -> Result<CopyOutcome, CopyFailure> {
        if let Err(error) = self.copy_preflight(source, target, &options) {
            return Err(self.contextualize_copy_failure(
                self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                ),
                source,
                target,
            ));
        }
        match self.spi.try_copy(CopyRequest::new(
            source,
            target,
            ResolvedCopyOptions::new(options.clone()),
        )) {
            Ok(CopyAttempt::Completed(outcome)) => {
                self.verify_completed_copy(outcome, &options, source, target)
            }
            Err(failure) => {
                let (error, state, stats) = failure.into_parts();
                Err(self.contextualize_copy_failure(
                    self.copy_failure(error, state, stats, None),
                    source,
                    target,
                ))
            }
            Ok(CopyAttempt::Declined(_)) => self
                .stream_copy_fallback(source, target, &options)
                .map_err(|failure| {
                    self.contextualize_copy_failure(failure, source, target)
                }),
        }
    }

    /// Verifies that a provider-completed fast path honors required outcome
    /// guarantees.
    #[allow(clippy::result_large_err)]
    fn verify_completed_copy(
        &self,
        outcome: CopyOutcome,
        options: &CopyOptions,
        source: &Path,
        target: &Path,
    ) -> Result<CopyOutcome, CopyFailure> {
        if let Some(message) = outcome.contract_violation(options) {
            return Err(self.contextualize_copy_failure(
                self.copy_failure(
                    FsError::new(
                        FsErrorKind::ProviderContractViolation,
                        FsOperation::Copy,
                        message,
                    ),
                    CopyFailureState::Published,
                    *outcome.stats(),
                    None,
                ),
                source,
                target,
            ));
        }
        if options.atomicity == crate::AtomicityRequirement::Required
            && outcome.atomicity() != crate::AchievedAtomicity::Atomic
        {
            return Err(self.contextualize_copy_failure(
                self.copy_failure(
                    FsError::new(
                        FsErrorKind::ProviderContractViolation,
                        FsOperation::Copy,
                        "provider reported non-atomic success for an atomic-required copy",
                    ),
                    CopyFailureState::Published,
                    *outcome.stats(),
                    None,
                ),
                source,
                target,
            ));
        }
        if options.durability == crate::DurabilityRequirement::Required
            && !outcome.durable()
        {
            return Err(self.contextualize_copy_failure(
                self.copy_failure(
                    FsError::new(
                        FsErrorKind::ProviderContractViolation,
                        FsOperation::Copy,
                        "provider reported non-durable success for a durability-required copy",
                    ),
                    CopyFailureState::Published,
                    *outcome.stats(),
                    None,
                ),
                source,
                target,
            ));
        }
        Ok(outcome)
    }

    /// Renames one resource through exactly one provider rename primitive.
    ///
    /// # Errors
    /// Returns [`RenameFailure`] with the provider-confirmed transition state.
    /// This method never implements rename through copy and delete.
    pub fn rename(
        &self,
        source: &Path,
        target: &Path,
        options: RenameOptions,
    ) -> Result<RenameOutcome, RenameFailure> {
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
            Ok(outcome)
                if options.atomicity == crate::AtomicityRequirement::Required
                    && outcome.atomicity() != crate::AchievedAtomicity::Atomic =>
            {
                Err(RenameFailure::new(
                    self.contract_error(
                        source,
                        FsOperation::Rename,
                        "provider reported non-atomic success for an atomic-required rename",
                    )
                    .with_target(target.clone()),
                    RenameFailureState::Renamed,
                ))
            }
            Ok(outcome) if outcome.method() == crate::PublicationMethod::CopyThenDelete => {
                Err(RenameFailure::new(
                    self.contract_error(
                        source,
                        FsOperation::Rename,
                        "provider returned copy-then-delete for rename",
                    )
                    .with_target(target.clone()),
                    RenameFailureState::Renamed,
                ))
            }
            Ok(outcome)
                if outcome.source() != source || outcome.target() != target =>
            {
                Err(RenameFailure::new(
                    self.contract_error(
                        source,
                        FsOperation::Rename,
                        "provider returned a rename outcome with different identities",
                    )
                    .with_target(target.clone()),
                    RenameFailureState::Indeterminate,
                ))
            }
            Ok(outcome) => Ok(outcome),
            Err(failure) => {
                let (error, state) = failure.into_parts();
                Err(RenameFailure::new(
                    error
                        .with_operation(FsOperation::Rename)
                        .with_missing_context(
                            source,
                            Some(target),
                            self.properties.info().provider_id(),
                        ),
                    state,
                ))
            }
        }
    }

    /// Opens a provider directory stream after local option validation.
    pub fn list(
        &self,
        path: &Path,
        options: ListOptions,
    ) -> FsResult<DirectoryStream> {
        self.validate_path(path, FsOperation::List)?;
        options.validate()?;
        self.require(FileSystemCapability::List, FsOperation::List, path)?;
        let options = ListOptions {
            page_size: self
                .properties
                .limits()
                .clamp_list_page_size(options.page_size),
            ..options
        };
        self.spi
            .list(ListRequest::new(
                path,
                ResolvedListOptions::new(options.clone()),
            ))
            .map(|opened| {
                DirectoryStream::new(
                    path.clone(),
                    opened.into_stream(),
                    options,
                    self.properties.info().provider_id(),
                )
            })
            .map_err(|error| self.enrich(error, path, FsOperation::List))
    }

    /// Opens a provider reader after local option validation.
    pub fn open_reader(
        &self,
        path: &Path,
        options: ReadOptions,
    ) -> FsResult<FileReader> {
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
        self.spi
            .open_reader(OpenReaderRequest::new(
                path,
                ResolvedReadOptions::new(options),
            ))
            .and_then(|opened| {
                let (info, reader) = opened.into_parts();
                self.validate_opened_info(
                    &info,
                    path,
                    FsOperation::OpenReader,
                )?;
                Ok(FileReader::new(info, reader))
            })
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))
    }

    /// Opens a provider writer after local option validation.
    pub fn open_writer(
        &self,
        path: &Path,
        options: WriteOptions,
    ) -> FsResult<FileWriter> {
        self.validate_path(path, FsOperation::OpenWriter)?;
        options.validate_against(self.properties.capabilities())?;
        self.require(
            FileSystemCapability::Write,
            FsOperation::OpenWriter,
            path,
        )?;
        let atomicity = options.atomicity;
        self.spi
            .open_writer(OpenWriterRequest::new(
                path,
                ResolvedWriteOptions::new(options),
            ))
            .and_then(|opened| {
                let (info, writer) = opened.into_parts();
                self.validate_opened_info(
                    &info,
                    path,
                    FsOperation::OpenWriter,
                )?;
                Ok(FileWriter::new(
                    info,
                    writer,
                    atomicity,
                    self.properties.info().provider_id(),
                    self.properties.limits().max_write_bytes().maximum(),
                ))
            })
            .map_err(|error| self.enrich(error, path, FsOperation::OpenWriter))
    }

    /// Creates a temporary file and binds its provider session to this facade.
    pub fn create_temp_file(
        &self,
        options: TempFileOptions,
    ) -> FsResult<TempFile> {
        self.require(
            FileSystemCapability::TempFile,
            FsOperation::CreateTemp,
            &Path::root(),
        )?;
        self.spi
            .create_temp_file(CreateTempFileRequest::new(options))
            .and_then(|opened| {
                let (info, mut session) = opened.into_parts();
                if let Err(error) = self.validate_temp_info(&info) {
                    return Err(match session.cleanup() {
                        Ok(()) => error,
                        Err(cleanup) => FsError::with_source(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::ValidateProviderOutcome,
                            "provider returned an invalid temporary identity and cleanup failed",
                            FsError::with_source(
                                cleanup.kind(),
                                cleanup.operation(),
                                "temporary cleanup failed",
                                error,
                            ),
                        ),
                    });
                }
                Ok(TempFile::new(self.clone(), info.path().clone(), session))
            })
            .map_err(|error| self.enrich(error, &Path::root(), FsOperation::CreateTemp))
    }

    /// Creates a temporary directory and binds its provider session to this
    /// facade.
    pub fn create_temp_directory(
        &self,
        options: TempDirectoryOptions,
    ) -> FsResult<TempDirectory> {
        self.require(
            FileSystemCapability::TempDirectory,
            FsOperation::CreateTemp,
            &Path::root(),
        )?;
        self.spi
            .create_temp_directory(CreateTempDirectoryRequest::new(options))
            .and_then(|opened| {
                let (info, mut session) = opened.into_parts();
                if let Err(error) = self.validate_temp_info(&info) {
                    return Err(match session.cleanup() {
                        Ok(()) => error,
                        Err(cleanup) => FsError::with_source(
                            FsErrorKind::ProviderContractViolation,
                            FsOperation::ValidateProviderOutcome,
                            "provider returned an invalid temporary identity and cleanup failed",
                            FsError::with_source(
                                cleanup.kind(),
                                cleanup.operation(),
                                "temporary cleanup failed",
                                error,
                            ),
                        ),
                    });
                }
                Ok(TempDirectory::new(
                    self.clone(),
                    info.path().clone(),
                    session,
                ))
            })
            .map_err(|error| self.enrich(error, &Path::root(), FsOperation::CreateTemp))
    }

    /// Creates a directory after local path validation.
    pub fn create_directory(
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
            .map_err(|error| {
                self.enrich(error, path, FsOperation::CreateDir)
            })?;
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
    pub fn delete_file(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false)
    }

    /// Deletes a directory after local option validation.
    pub fn delete_directory(
        &self,
        path: &Path,
        options: DeleteOptions,
    ) -> FsResult<DeleteOutcome> {
        self.delete(path, options, true)
    }

    /// Validates and dispatches one deletion primitive.
    fn delete(
        &self,
        path: &Path,
        options: DeleteOptions,
        directory: bool,
    ) -> FsResult<DeleteOutcome> {
        self.validate_path(path, FsOperation::Delete)?;
        options.validate_against(self.properties.capabilities())?;
        self.require(FileSystemCapability::Delete, FsOperation::Delete, path)?;
        let missing_ok = options.missing_ok;
        let outcome = if directory {
            self.spi.delete_directory(DeleteDirectoryRequest::new(
                path,
                ResolvedDeleteOptions::new(options),
            ))
        } else {
            self.spi.delete_file(DeleteFileRequest::new(
                path,
                ResolvedDeleteOptions::new(options),
            ))
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

    /// Performs validation common to provider copy dispatch and fallback
    /// selection.
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
                FsErrorKind::InvalidOptions,
                FsOperation::Copy,
                "copy source and target must differ",
            )
            .with_path(source.clone())
            .with_target(target.clone()));
        }
        Ok(())
    }

    /// Applies the deliberately narrow fallback allowlist before opening either
    /// stream handle.
    #[allow(clippy::result_large_err)]
    fn stream_copy_fallback(
        &self,
        source: &Path,
        target: &Path,
        options: &CopyOptions,
    ) -> Result<CopyOutcome, CopyFailure> {
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
            return Err(self.copy_failure(
                FsError::new(
                    FsErrorKind::RequirementNotMet,
                    FsOperation::Copy,
                    "declined copy cannot use the stream fallback for these options",
                ),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
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
                self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                )
            })?;
        let metadata = self.stat(source).map_err(|error| {
            self.copy_failure(
                error,
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            )
        })?;
        if !matches!(metadata.kind, FileKind::File | FileKind::Object) {
            return Err(self.copy_failure(
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::Copy,
                    "stream fallback only supports regular files and objects",
                )
                .with_path(source.clone()),
                CopyFailureState::Unchanged,
                CopyStats::default(),
                None,
            ));
        }
        if let Some(length) = metadata.len {
            if let Err(error) = self
                .properties
                .limits()
                .validate_read_range(source, Some(length))
            {
                return Err(self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                ));
            }
            let length = match usize::try_from(length) {
                Ok(length) => length,
                Err(_) => {
                    return Err(self.copy_failure(
                        FsError::new(
                            FsErrorKind::ResourceLimitExceeded,
                            FsOperation::Copy,
                            "source length cannot fit in a write session",
                        ),
                        CopyFailureState::Unchanged,
                        CopyStats::default(),
                        None,
                    ));
                }
            };
            if let Err(error) =
                self.properties.limits().validate_write_size(target, length)
            {
                return Err(self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                ));
            }
        }
        let mut reader = match self.open_reader(source, ReadOptions::default())
        {
            Ok(reader) => reader,
            Err(error) => {
                return Err(self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                ));
            }
        };
        let writer_options = WriteOptions {
            disposition: WriteDisposition::CreateNew,
            atomicity: options.atomicity,
            ..WriteOptions::default()
        };
        let mut writer = match self.open_writer(target, writer_options) {
            Ok(writer) => writer,
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
                return Err(self.copy_failure(
                    error,
                    CopyFailureState::Unchanged,
                    CopyStats::default(),
                    None,
                ));
            }
        };
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 8192];
        loop {
            let read = match Input::read(&mut reader, &mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    return Err(self.copy_failure(
                        self.io_error(source, FsOperation::Read, error),
                        CopyFailureState::PartiallyPublished,
                        fallback_failure_stats(writer.written_bytes()),
                        Some(writer),
                    ));
                }
            };
            if read == 0 {
                break;
            }
            if let Err(error) =
                Output::write_fully(&mut writer, &buffer[..read])
            {
                return Err(self.copy_failure(
                    self.io_error(target, FsOperation::Write, error),
                    CopyFailureState::PartiallyPublished,
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
            bytes = bytes.saturating_add(read as u64);
        }
        if let Err(error) = Output::flush(&mut writer) {
            return Err(self.copy_failure(
                self.io_error(target, FsOperation::Write, error),
                CopyFailureState::PartiallyPublished,
                fallback_failure_stats(writer.written_bytes()),
                Some(writer),
            ));
        }
        let write_outcome = match writer.commit() {
            Ok(outcome) => outcome,
            Err(failure) => {
                let (error, state) = failure.into_parts();
                let state = match state {
                    WriteFailureState::Published => CopyFailureState::Published,
                    WriteFailureState::Indeterminate => {
                        CopyFailureState::Indeterminate
                    }
                    WriteFailureState::RetryableNotPublished
                    | WriteFailureState::NotPublished => {
                        CopyFailureState::PartiallyPublished
                    }
                };
                return Err(self.copy_failure(
                    error,
                    state,
                    fallback_failure_stats(writer.written_bytes()),
                    Some(writer),
                ));
            }
        };
        Ok(CopyOutcome::streamed_fallback(
            CopyStats {
                files: 1,
                bytes,
                ..CopyStats::default()
            },
            write_outcome.atomicity,
        ))
    }

    /// Performs validation before the single rename provider call.
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

    /// Builds a contextual typed copy failure.
    fn copy_failure(
        &self,
        error: FsError,
        state: CopyFailureState,
        stats: CopyStats,
        writer: Option<FileWriter>,
    ) -> CopyFailure {
        CopyFailure::new(
            error.with_operation(FsOperation::Copy),
            state,
            stats,
            writer,
        )
    }

    /// Adds source, target, and provider facts required by public copy
    /// failures.
    fn contextualize_copy_failure(
        &self,
        failure: CopyFailure,
        source: &Path,
        target: &Path,
    ) -> CopyFailure {
        let (error, state, stats, writer) = failure.into_parts();
        CopyFailure::new(
            error.with_missing_context(
                source,
                Some(target),
                self.properties.info().provider_id(),
            ),
            state,
            stats,
            writer,
        )
    }

    /// Adds source, target, and provider facts required by public rename
    /// failures.
    fn contextualize_rename_failure(
        &self,
        failure: RenameFailure,
        source: &Path,
        target: &Path,
    ) -> RenameFailure {
        let (error, state) = failure.into_parts();
        RenameFailure::new(
            error.with_missing_context(
                source,
                Some(target),
                self.properties.info().provider_id(),
            ),
            state,
        )
    }

    /// Validates a path against the cached provider snapshot before I/O.
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

    /// Requires an advertised capability before I/O.
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
                FsErrorKind::UnsupportedCapability,
                operation,
                "filesystem capability is not supported",
            )
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id())
            .with_required_capability(capability))
        }
    }

    /// Adds missing public error context to a provider error.
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

    /// Builds a provider-contract error for an invalid outcome.
    fn contract_error(
        &self,
        path: &Path,
        operation: FsOperation,
        message: &str,
    ) -> FsError {
        FsError::new(FsErrorKind::ProviderContractViolation, operation, message)
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id())
    }

    /// Validates the identity the provider attached to an opened file handle.
    fn validate_opened_info(
        &self,
        info: &crate::OpenedFileInfo,
        path: &Path,
        operation: FsOperation,
    ) -> FsResult<()> {
        if info.filesystem_id() != self.properties.info().id()
            || info.path() != path
        {
            return Err(self.contract_error(
                path,
                FsOperation::ValidateProviderOutcome,
                "provider returned an opened handle with a different identity",
            ));
        }
        let _ = operation;
        Ok(())
    }

    /// Validates the filesystem identity captured for a newly-created temporary
    /// resource.
    fn validate_temp_info(&self, info: &crate::OpenedFileInfo) -> FsResult<()> {
        if info.filesystem_id() != self.properties.info().id() {
            return Err(self.contract_error(
                info.path(),
                FsOperation::ValidateProviderOutcome,
                "provider returned a temporary handle for a different filesystem",
            ));
        }
        self.validate_path(info.path(), FsOperation::CreateTemp)
            .map_err(|_| {
                self.contract_error(
                    info.path(),
                    FsOperation::ValidateProviderOutcome,
                    "provider returned a temporary handle with an invalid logical path",
                )
            })?;
        Ok(())
    }

    /// Validates a temporary-resource persistence request before provider
    /// effects.
    pub(crate) fn preflight_temp_persist(
        &self,
        source: &Path,
        target: &Path,
        options: &crate::PersistOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::PersistTemp)?;
        self.validate_path(target, FsOperation::PersistTemp)?;
        options.validate_against(self.properties.capabilities())?;
        Ok(())
    }

    /// Reads one file into memory up to `max_bytes` after opening a reader.
    pub fn read_all(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<Vec<u8>> {
        let mut reader = self.open_reader(path, options)?;
        let mut result = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read =
                Input::read(&mut reader, &mut buffer).map_err(|error| {
                    self.io_error(path, FsOperation::Read, error)
                })?;
            if read == 0 {
                return Ok(result);
            }
            if result.len().saturating_add(read) > max_bytes {
                return Err(FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Read,
                    "read exceeds maximum byte count",
                )
                .with_path(path.clone()));
            }
            result.extend_from_slice(&buffer[..read]);
        }
    }

    /// Writes all bytes and retains the writer if transfer or commit fails.
    pub fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<crate::WriteOutcome, WriteAllFailure> {
        if let Err(error) = self
            .properties
            .limits()
            .validate_write_size(path, bytes.len())
        {
            return Err(WriteAllFailure::new(error, None));
        }
        let mut writer = self
            .open_writer(path, options)
            .map_err(|error| WriteAllFailure::new(error, None))?;
        if let Err(error) = Output::write_fully(&mut writer, bytes)
            .and_then(|_| Output::flush(&mut writer))
        {
            return Err(WriteAllFailure::new(
                self.io_error(path, FsOperation::Write, error),
                Some(writer),
            ));
        }
        writer.commit().map_err(|failure| {
            let (error, _) = failure.into_parts();
            WriteAllFailure::new(error, Some(writer))
        })
    }

    /// Creates a contextual filesystem error from an I/O stream failure.
    fn io_error(
        &self,
        path: &Path,
        operation: FsOperation,
        error: IoError,
    ) -> FsError {
        FsError::from_stream_io(error, operation, path)
    }
}

/// Builds accurate progress for a fallback failure after its destination writer
/// was opened.
fn fallback_failure_stats(bytes: u64) -> CopyStats {
    CopyStats {
        bytes,
        failed: 1,
        ..CopyStats::default()
    }
}
