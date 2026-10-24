// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validating synchronous filesystem facade.

use std::sync::Arc;

use crate::copy::CopyAssessment;
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
use crate::directory::ListScope;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::error::OpenFailure;
use crate::error::OpenFailureStage;
use crate::facade::facade_core::FacadeCore;
use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::metadata::OpenedFileInfo;
use crate::metadata::WriteOutcome;
use crate::path::Path;
use crate::read::FileReader;
use crate::read::PrefixReadOutcome;
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
use crate::temp::PersistOptions;
use crate::temp::RejectedTempResource;
use crate::temp::TempDirectory;
use crate::temp::TempFile;
use crate::temp::TempOptions;
use crate::write::FileWriter;
use crate::write::RejectedWriter;
use crate::write::WriteAllFailure;
use crate::write::WriteOperation;
use crate::write::WriteOptions;

/// Application-facing synchronous filesystem facade.
///
/// # Examples
///
/// The example runs against an isolated in-memory fixture. Applications obtain
/// their configured facade from a provider or registry integration.
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # let filesystem = support::rustdoc_provider::filesystem();
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadOptions;
///
/// let path = Path::parse("/report")?;
/// let bytes = filesystem.read_prefix(&path, ReadOptions::default(), 3)?;
/// assert_eq!(b"rep", bytes.bytes());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
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
    ///
    /// # Parameters
    /// - `spi`: Provider implementation whose properties and operations back
    ///   the facade.
    ///
    /// # Returns
    /// The facade when the provider property snapshot is valid.
    ///
    /// # Errors
    /// Returns the provider property validation error when its snapshot is
    /// inconsistent.
    #[inline]
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: FileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    ///
    /// # Parameters
    /// - `spi`: Shared provider implementation retained by the facade.
    ///
    /// # Returns
    /// The facade when the provider property snapshot is valid.
    ///
    /// # Errors
    /// Returns the provider property validation error when its snapshot is
    /// inconsistent.
    #[inline]
    pub fn from_shared_spi(spi: Arc<dyn FileSystemSpi>) -> FsResult<Self> {
        let core = FacadeCore::new(spi.properties())?;
        Ok(Self {
            spi,
            core: Arc::new(core),
        })
    }

    /// Returns the immutable property snapshot without provider I/O.
    ///
    /// # Returns
    /// The validated properties captured when this facade was constructed.
    #[inline]
    #[must_use]
    pub fn properties(&self) -> &FileSystemProperties {
        self.core.properties()
    }

    /// Assesses copy routes without performing provider I/O.
    ///
    /// # Errors
    /// Returns an invalid-options or requirement error when no provider or
    /// stream execution route can satisfy the request.
    ///
    /// # Parameters
    /// - `source`: Logical source path.
    /// - `target`: Logical destination path.
    /// - `options`: Copy policy to assess.
    ///
    /// # Returns
    /// The selected route and the first statically rejected fallback, when one
    /// exists.
    pub fn assess_copy(&self, source: &Path, target: &Path, options: &CopyOptions) -> FsResult<CopyAssessment> {
        self.core.assess_copy(source, target, options)
    }

    /// Reads metadata after all local validation succeeds.
    ///
    /// # Errors
    /// Returns the provider or validation error when `path` is invalid, the
    /// provider cannot stat it, or the provider returns a mismatched path.
    ///
    /// # Parameters
    /// - `path`: Logical resource path to inspect.
    ///
    /// # Returns
    /// Metadata for the requested path after provider identity validation.
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
    ///
    /// # Errors
    /// Returns the original filesystem error for validation, provider, or
    /// provider-contract failures other than `NotFound`.
    ///
    /// # Parameters
    /// - `path`: Logical resource path to check.
    ///
    /// # Returns
    /// `true` when metadata is returned, or `false` for an explicit not-found
    /// response.
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
    ///
    /// # Parameters
    /// - `source`: Logical resource to copy.
    /// - `target`: Logical destination path.
    /// - `options`: Copy behavior and fallback policy.
    ///
    /// # Returns
    /// The confirmed copy outcome.
    #[allow(clippy::result_large_err)]
    pub fn copy(&self, source: &Path, target: &Path, options: CopyOptions) -> Result<CopyOutcome, CopyFailure> {
        CopyOperation::new(self, source, target, options).execute()
    }

    /// Renames one resource through exactly one provider rename primitive.
    ///
    /// # Errors
    /// Returns [`RenameFailure`] with the provider-confirmed transition state.
    /// This method never implements rename through copy and delete.
    ///
    /// # Parameters
    /// - `source`: Existing logical resource path.
    /// - `target`: Destination logical path.
    /// - `options`: Rename policy passed to the provider.
    ///
    /// # Returns
    /// The provider-confirmed rename outcome.
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
    ///
    /// # Errors
    /// Returns an invalid-options, missing-capability, or provider error when
    /// the scope or options cannot be served.
    ///
    /// # Parameters
    /// - `scope`: Directory scope to enumerate.
    /// - `options`: Listing behavior and limits.
    ///
    /// # Returns
    /// A stream of validated directory entries.
    pub fn list(&self, scope: &ListScope, options: ListOptions) -> FsResult<DirectoryStream> {
        DirectoryOperation::new(self).list(scope, options)
    }

    /// Opens a provider reader after local option validation.
    ///
    /// # Errors
    /// Returns an invalid-path, unsupported-capability, provider, or
    /// provider-contract error when the reader cannot be opened safely.
    ///
    /// # Parameters
    /// - `path`: Logical file path to open.
    /// - `options`: Read range and behavior.
    ///
    /// # Returns
    /// A reader bound to the validated provider handle.
    pub fn open_reader(&self, path: &Path, options: ReadOptions) -> FsResult<FileReader> {
        self.open_reader_resolved(path, ResolvedReadOptions::new(options))
    }

    /// Opens a provider writer after local option validation.
    ///
    /// # Errors
    /// Returns [`OpenFailure`] identifying preflight, provider-open, or
    /// provider-contract failure; a retained rejected writer is available when
    /// provider cleanup is required.
    ///
    /// # Parameters
    /// - `path`: Logical file path to open for writing.
    /// - `options`: Write and publication behavior.
    ///
    /// # Returns
    /// The writer, or an `OpenFailure` retaining cleanup state when needed.
    pub fn open_writer(&self, path: &Path, options: WriteOptions) -> Result<FileWriter, OpenFailure<RejectedWriter>> {
        self.core
            .validate_write_request(path, &options)
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        let atomicity = options.atomicity();
        let durability = options.durability();
        let opened = self
            .spi
            .open_writer(OpenWriterRequest::new(path, ResolvedWriteOptions::new(options)))
            .map_err(|error| {
                OpenFailure::new(
                    self.enrich(error, path, FsOperation::OpenWriter),
                    OpenFailureStage::ProviderOpen,
                    None,
                )
            })?;
        let (info, session) = opened.into_parts();
        if let Err(error) = self.validate_opened_info(&info, path) {
            return Err(OpenFailure::new(
                error,
                OpenFailureStage::OutcomeValidation,
                Some(RejectedWriter::new(
                    session,
                    self.properties().info().provider_id(),
                    Some(path.clone()),
                )),
            ));
        }
        Ok(FileWriter::new(
            info,
            session,
            atomicity,
            durability,
            self.properties().info().provider_id(),
            self.properties().limits().max_write_bytes().maximum(),
        ))
    }

    /// Creates a temporary file and binds its provider session to this facade.
    ///
    /// # Errors
    /// Returns [`OpenFailure`] when preflight, provider creation, or temporary
    /// identity validation fails; the recovery value may retain cleanup work.
    ///
    /// # Parameters
    /// - `options`: Parent and naming policy for the temporary file.
    ///
    /// # Returns
    /// A temporary file handle bound to this facade.
    pub fn create_temp_file(&self, options: TempOptions) -> Result<TempFile, OpenFailure<RejectedTempResource>> {
        let parent = options.parent().cloned();
        self.core
            .validate_temp_parent(parent.as_ref())
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        self.core
            .require(FileSystemCapability::TempFile, FsOperation::CreateTemp, parent.as_ref())
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        let opened = self
            .spi
            .create_temp_file(CreateTempFileRequest::new(options))
            .map_err(|error| {
                OpenFailure::new(
                    self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp),
                    OpenFailureStage::ProviderOpen,
                    None,
                )
            })?;
        let (info, session) = opened.into_parts();
        if let Err(cause) = self.validate_temp_info(&info, FileKind::File) {
            let error = FsError::with_source(
                FsErrorKind::ProviderContractViolation,
                FsOperation::ValidateProviderOutcome,
                "provider returned an invalid temporary identity",
                cause,
            )
            .with_provider(self.properties().info().provider_id());
            let error = match parent.as_ref() {
                Some(path) => error.with_path(path.clone()),
                None => error,
            };
            return Err(OpenFailure::new(
                error,
                OpenFailureStage::OutcomeValidation,
                Some(RejectedTempResource::new(
                    session,
                    self.properties().info().provider_id(),
                    parent,
                )),
            ));
        }
        Ok(TempFile::new(self.clone(), info.path().clone(), session))
    }

    /// Creates a temporary directory and binds its provider session to this
    /// facade.
    ///
    /// # Errors
    /// Returns [`OpenFailure`] when preflight, provider creation, or temporary
    /// identity validation fails; the recovery value may retain cleanup work.
    ///
    /// # Parameters
    /// - `options`: Parent and naming policy for the temporary directory.
    ///
    /// # Returns
    /// A temporary directory handle bound to this facade.
    pub fn create_temp_directory(
        &self,
        options: TempOptions,
    ) -> Result<TempDirectory, OpenFailure<RejectedTempResource>> {
        let parent = options.parent().cloned();
        self.core
            .validate_temp_parent(parent.as_ref())
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        self.core
            .require(
                FileSystemCapability::TempDirectory,
                FsOperation::CreateTemp,
                parent.as_ref(),
            )
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        let opened = self
            .spi
            .create_temp_directory(CreateTempDirectoryRequest::new(options))
            .map_err(|error| {
                OpenFailure::new(
                    self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp),
                    OpenFailureStage::ProviderOpen,
                    None,
                )
            })?;
        let (info, session) = opened.into_parts();
        if let Err(cause) = self.validate_temp_info(&info, FileKind::Directory) {
            let error = FsError::with_source(
                FsErrorKind::ProviderContractViolation,
                FsOperation::ValidateProviderOutcome,
                "provider returned an invalid temporary identity",
                cause,
            )
            .with_provider(self.properties().info().provider_id());
            let error = match parent.as_ref() {
                Some(path) => error.with_path(path.clone()),
                None => error,
            };
            return Err(OpenFailure::new(
                error,
                OpenFailureStage::OutcomeValidation,
                Some(RejectedTempResource::new(
                    session,
                    self.properties().info().provider_id(),
                    parent,
                )),
            ));
        }
        Ok(TempDirectory::new(self.clone(), info.path().clone(), session))
    }

    /// Creates a directory after local path validation.
    ///
    /// # Errors
    /// Returns an invalid-path, missing-capability, provider, or provider-
    /// contract error when the directory cannot be created as requested.
    ///
    /// # Parameters
    /// - `path`: Logical directory path to create.
    /// - `options`: Creation and parent handling policy.
    ///
    /// # Returns
    /// The provider-confirmed directory creation outcome.
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
    ///
    /// # Errors
    /// Returns an invalid-options, missing-capability, provider, or
    /// provider-contract error when deletion cannot be confirmed.
    ///
    /// # Parameters
    /// - `path`: Logical file path to delete.
    /// - `options`: Deletion policy.
    ///
    /// # Returns
    /// The provider-confirmed deletion outcome.
    #[inline]
    pub fn delete_file(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false)
    }

    /// Deletes a directory after local option validation.
    ///
    /// # Errors
    /// Returns an invalid-options, missing-capability, provider, or
    /// provider-contract error when deletion cannot be confirmed.
    ///
    /// # Parameters
    /// - `path`: Logical directory path to delete.
    /// - `options`: Deletion policy.
    ///
    /// # Returns
    /// The provider-confirmed deletion outcome.
    #[inline]
    pub fn delete_directory(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, true)
    }

    /// Reads one file into memory up to `max_bytes` after opening a reader.
    ///
    /// The byte cap applies to bytes actually read, even when opened metadata
    /// overestimates the resource length.
    ///
    /// # Errors
    /// Returns the reader or read error when validation, opening, or bounded
    /// reading fails.
    ///
    /// # Parameters
    /// - `path`: Logical file path to read.
    /// - `options`: Read range and behavior.
    /// - `max_bytes`: Maximum number of bytes allowed in memory.
    ///
    /// # Returns
    /// The complete contents when the file fits within the byte cap.
    pub fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        ReadOperation::new(self).read_all(path, options, max_bytes)
    }

    /// Reads at most max_bytes from a file without requiring a complete read.
    ///
    /// # Errors
    /// Returns the reader or read error when validation, opening, or bounded
    /// reading fails.
    ///
    /// # Parameters
    /// - `path`: Logical file path to read.
    /// - `options`: Read range and behavior.
    /// - `max_bytes`: Maximum number of bytes to consume.
    ///
    /// # Returns
    /// The bytes read and the termination reason, which distinguishes end of
    /// file from reaching the byte limit.
    pub fn read_prefix(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<PrefixReadOutcome> {
        ReadOperation::new(self).read_prefix(path, options, max_bytes)
    }

    /// Writes all bytes and retains the writer if transfer or commit fails.
    ///
    /// # Errors
    /// Returns [`WriteAllFailure`] with the confirmed byte count and retained
    /// writer when validation, transfer, or publication fails.
    ///
    /// # Parameters
    /// - `path`: Logical destination path.
    /// - `bytes`: Complete payload to write.
    /// - `options`: Write and publication behavior.
    ///
    /// # Returns
    /// The confirmed write outcome, or a failure retaining the writer and
    /// confirmed transfer state.
    pub fn write_all(&self, path: &Path, bytes: &[u8], options: WriteOptions) -> Result<WriteOutcome, WriteAllFailure> {
        WriteOperation::new(self).write_all(path, bytes, options)
    }

    /// Validates a write request without opening a provider session.
    ///
    /// # Parameters
    /// - `path`: Logical destination path to validate.
    /// - `options`: Write configuration checked against cached capabilities and
    ///   limits.
    ///
    /// # Returns
    /// `Ok(())` when the request is locally valid.
    ///
    /// # Errors
    /// Returns the path, option, capability, or limit validation error.
    pub fn validate_write(&self, path: &Path, options: &WriteOptions) -> FsResult<()> {
        self.core.validate_write_request(path, options)
    }

    /// Returns shared deterministic facade policy to operation objects.
    ///
    /// # Returns
    /// The cached policy shared by internal operation objects.
    #[inline]
    pub(crate) fn core(&self) -> &FacadeCore {
        &self.core
    }

    /// Returns the synchronous provider implementation to operation objects.
    ///
    /// # Returns
    /// The provider implementation retained by this facade.
    #[inline]
    pub(crate) fn spi(&self) -> &dyn FileSystemSpi {
        self.spi.as_ref()
    }

    /// Opens a facade-validated reader with an internal resolved option hint.
    ///
    /// # Parameters
    /// - `path`: Logical file path to open.
    /// - `resolved`: Validated read options, including any internal hint.
    ///
    /// # Returns
    /// A reader bound to the validated provider handle.
    ///
    /// # Errors
    /// Returns a path, option, capability, provider, or provider-contract
    /// error when the reader cannot be opened safely.
    pub(crate) fn open_reader_resolved(&self, path: &Path, resolved: ResolvedReadOptions) -> FsResult<FileReader> {
        let options = resolved.options().clone();
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
            .open_reader(OpenReaderRequest::new(path, resolved))
            .and_then(|opened| {
                let (info, reader) = opened.into_parts();
                self.validate_opened_info(&info, path)?;
                Ok(FileReader::new(info, reader))
            })
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))
    }

    /// Validates a temporary-resource persistence request before provider
    /// effects.
    ///
    /// This crate-visible preflight preserves source and target context before
    /// a temporary resource is handed to its provider.
    ///
    /// # Parameters
    /// - `source`: Temporary resource path.
    /// - `target`: Requested persistence destination.
    /// - `options`: Persistence behavior to validate.
    ///
    /// # Returns
    /// `Ok(())` when the paths and options pass local validation.
    ///
    /// # Errors
    /// Returns a path or option validation error with source and target
    /// context.
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
            })?;
        Ok(())
    }

    /// Validates a provider-generated target reported by temporary keep.
    ///
    /// # Parameters
    /// - `source`: Temporary resource path used for error context.
    /// - `target`: Provider-reported destination path.
    ///
    /// # Returns
    /// `Ok(())` when the provider-reported target is a valid logical path.
    ///
    /// # Errors
    /// Returns a provider-contract error when the target path is invalid.
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

    /// Validates and dispatches one deletion primitive.
    ///
    /// # Parameters
    /// - `path`: Logical target path.
    /// - `options`: Deletion policy.
    /// - `directory`: Whether to invoke the directory deletion primitive.
    ///
    /// # Returns
    /// The provider-confirmed deletion outcome.
    ///
    /// # Errors
    /// Returns validation, capability, provider, or provider-contract errors.
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
    ///
    /// # Parameters
    /// - `source`: Existing logical resource path.
    /// - `target`: Requested destination path.
    /// - `options`: Rename policy.
    ///
    /// # Returns
    /// `Ok(())` when the request passes local validation.
    ///
    /// # Errors
    /// Returns a path, option, capability, or same-path validation error.
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
    ///
    /// # Parameters
    /// - `failure`: Rename failure to enrich.
    /// - `source`: Original source path.
    /// - `target`: Requested destination path.
    ///
    /// # Returns
    /// The failure with missing source, target, and provider context filled.
    fn contextualize_rename_failure(&self, failure: RenameFailure, source: &Path, target: &Path) -> RenameFailure {
        let (error, state) = failure.into_parts();
        RenameFailure::new(
            error.with_missing_context(source, Some(target), self.properties().info().provider_id()),
            state,
        )
    }

    /// Validates a path against the cached provider snapshot before I/O.
    ///
    /// # Parameters
    /// - `path`: Logical resource path to validate.
    /// - `operation`: Operation associated with validation errors.
    ///
    /// # Returns
    /// `Ok(())` when the path is valid for this provider.
    ///
    /// # Errors
    /// Returns the path validation error.
    fn validate_path(&self, path: &Path, operation: FsOperation) -> FsResult<()> {
        self.core.validate_path(path, operation)
    }

    /// Requires an advertised capability before I/O.
    ///
    /// # Parameters
    /// - `capability`: Capability required by the operation.
    /// - `operation`: Operation associated with a missing-capability error.
    /// - `path`: Logical resource path included in error context.
    ///
    /// # Returns
    /// `Ok(())` when the provider advertises the capability.
    ///
    /// # Errors
    /// Returns a missing-capability error when the provider lacks it.
    fn require(&self, capability: FileSystemCapability, operation: FsOperation, path: &Path) -> FsResult<()> {
        self.core.require(capability, operation, Some(path))
    }

    /// Adds missing public error context to a provider error.
    ///
    /// # Parameters
    /// - `error`: Provider error to enrich.
    /// - `path`: Logical resource path associated with the error.
    /// - `operation`: Operation associated with the error.
    ///
    /// # Returns
    /// The error with missing path, operation, and provider context filled.
    fn enrich(&self, error: FsError, path: &Path, operation: FsOperation) -> FsError {
        self.core.enrich(error, Some(path), operation)
    }

    /// Builds a provider-contract error for an invalid outcome.
    ///
    /// # Parameters
    /// - `path`: Path associated with the invalid outcome.
    /// - `operation`: Operation that produced the outcome.
    /// - `message`: Description of the provider contract violation.
    ///
    /// # Returns
    /// A provider-contract error containing the supplied context.
    fn contract_error(&self, path: &Path, operation: FsOperation, message: &str) -> FsError {
        self.core.contract_error(path, operation, message)
    }

    /// Validates the identity the provider attached to an opened file handle.
    ///
    /// # Parameters
    /// - `info`: Provider-reported opened-file identity.
    /// - `path`: Requested logical resource path.
    ///
    /// # Returns
    /// `Ok(())` when filesystem and path identities match.
    ///
    /// # Errors
    /// Returns a provider-contract error when either identity differs.
    fn validate_opened_info(&self, info: &OpenedFileInfo, path: &Path) -> FsResult<()> {
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
    ///
    /// # Parameters
    /// - `info`: Provider-reported temporary-resource identity.
    /// - `expected_kind`: Resource kind requested from the provider.
    ///
    /// # Returns
    /// `Ok(())` when filesystem, path, and resource kind are valid.
    ///
    /// # Errors
    /// Returns a provider-contract error when any identity is inconsistent.
    fn validate_temp_info(&self, info: &OpenedFileInfo, expected_kind: FileKind) -> FsResult<()> {
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
}
