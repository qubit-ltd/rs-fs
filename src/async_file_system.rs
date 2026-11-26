// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem facade construction boundary.

use std::sync::Arc;

use qubit_operation::OperationScope;

use crate::AsyncFsMutationOperation;
use crate::FsEffectSlotPool;
use crate::FsOwnedCopyRequest;
use crate::FsOwnedRenameRequest;
use crate::FsOwnedTreePublishRequest;
use crate::FsOwnedWriteRequest;
use crate::copy::AsyncCopyFailure;
use crate::copy::AsyncCopyOperation;
use crate::copy::CopyAssessment;
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
use crate::directory::ListScope;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::error::OpenFailure;
use crate::error::OpenFailureStage;
use crate::execution::FsExecutionBinding;
use crate::execution::FsOwnedReadRequest;
use crate::facade::facade_core::FacadeCore;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::path::Path;
use crate::read::AsyncFileReader;
use crate::read::AsyncRandomAccessReader;
use crate::read::AsyncReadOperation;
use crate::read::ObservedReadError;
use crate::read::RandomAccessReadPolicy;
use crate::read::ReadIoObserver;
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
use crate::spi::ManagedFsMutationSpi;
use crate::spi::ManagedFsReadSpi;
use crate::spi::ManagedFsTreeSpi;
use crate::spi::OpenReaderRequest;
use crate::spi::OpenedAsyncReader;
use crate::spi::OpenWriterRequest;
use crate::spi::ProviderOperation;
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
use crate::temp::RejectedAsyncTempResource;
use crate::temp::TempOptions;
use crate::write::AsyncFileWriter;
use crate::write::AsyncWriteAllOperation;
use crate::write::AsyncWriteAllOperationFailure;
use crate::write::RejectedAsyncWriter;
use crate::write::WriteOptions;

/// Application-facing asynchronous filesystem facade. Managed submissions
/// are explicitly bound to a caller-owned parent scope.
///
/// It validates provider boundaries, exposes asynchronous operations, and owns
/// the cancellation-safe copy operation entry point.
///
/// # Examples
///
/// Applications obtain their configured facade from a provider or registry
/// integration. The function below reads at most three bytes; the result may
/// be shorter when the file contains fewer bytes.
///
/// ```rust
/// use qubit_fs::{AsyncFileSystem, FsResult, Path};
/// use qubit_fs::read::ReadOptions;
///
/// async fn read_report_prefix(filesystem: &AsyncFileSystem, path: &Path) -> FsResult<Vec<u8>> {
///     let prefix = filesystem.read_prefix(path, ReadOptions::default(), 3).await?;
///     Ok(prefix.into_bytes())
/// }
/// ```
#[derive(Clone)]
pub struct AsyncFileSystem {
    /// Provider implementation receiving validated asynchronous requests.
    spi: Arc<dyn AsyncFileSystemSpi>,
    /// Shared immutable state and deterministic preflight policy.
    core: Arc<FacadeCore>,
}

impl AsyncFileSystem {
    /// Constructs a facade and caches one validated provider snapshot.
    ///
    /// # Type Parameters
    ///
    /// - `S`: Concrete provider SPI implementation type.
    ///
    /// # Parameters
    ///
    /// - `spi`: Provider implementation whose properties and operations back
    ///   this facade.
    /// # Returns
    /// A facade backed by the validated provider snapshot.
    /// # Errors
    /// Returns a provider validation error when its properties are invalid.
    #[inline]
    pub fn from_spi<S>(spi: S) -> FsResult<Self>
    where
        S: AsyncFileSystemSpi + 'static,
    {
        Self::from_shared_spi(Arc::new(spi))
    }

    /// Constructs a facade from a shared provider implementation.
    ///
    /// # Parameters
    ///
    /// - `spi`: Provider implementation whose properties and operations back
    ///   this facade.
    /// # Returns
    /// A facade backed by the shared provider and validated snapshot.
    /// # Errors
    /// Returns a provider validation error when its properties are invalid.
    #[inline]
    pub fn from_shared_spi(spi: Arc<dyn AsyncFileSystemSpi>) -> FsResult<Self> {
        let core = FacadeCore::new(spi.properties())?;
        Ok(Self {
            spi,
            core: Arc::new(core),
        })
    }

    /// Submits a bounded managed read as a child of `parent`.
    ///
    /// Uses the explicit provider for managed preparation and provider-specific
    /// preflight; this facade does not use its own SPI or cached properties.
    /// The parent manager decides admission and owns the child. A parent stop
    /// propagates to the child, without proving provider effects were rolled
    /// back.
    ///
    /// # Parameters
    /// - `parent`: Open operation scope that owns this child submission.
    /// - `provider`: Provider that prepares the owned read request.
    /// - `path`: Path to read.
    /// - `options`: Read byte and consistency limits.
    ///
    /// # Returns
    /// A managed read operation submitted under `parent`.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` with the original request when provider
    /// preparation or manager admission rejects the child, including when
    /// the parent is closed.
    #[inline]
    pub fn submit_read_all_in(
        &self,
        parent: &OperationScope,
        provider: &dyn ManagedFsReadSpi,
        path: Path,
        options: crate::read::ReadAllOptions,
    ) -> Result<crate::AsyncFsOperation<Vec<u8>>, crate::FsSubmitFailure<FsOwnedReadRequest>> {
        FsExecutionBinding::new(parent.manager()).submit_read_all_async(provider, FsOwnedReadRequest { path, options })
    }

    /// Submits a managed write under the supplied parent operation scope.
    ///
    /// Uses the explicit provider for managed preparation and provider-specific
    /// preflight; this facade does not use its own SPI or cached properties.
    /// The parent manager decides admission and owns the child. A parent stop
    /// propagates to the child, without proving provider effects were rolled
    /// back.
    ///
    /// # Parameters
    /// - `parent`: Open operation scope that owns this child submission.
    /// - `provider`: Provider that prepares the owned write request.
    /// - `request`: Owned write parameters.
    /// - `effects`: Bounded pool used to reserve final effect evidence.
    ///
    /// # Returns
    /// A managed write operation submitted under `parent`.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` with the original request when provider
    /// preparation or manager admission rejects the child, including when
    /// the parent is closed.
    #[inline]
    pub fn submit_write_in(
        &self,
        parent: &OperationScope,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedWriteRequest,
        effects: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::metadata::WriteOutcome>, crate::FsSubmitFailure<FsOwnedWriteRequest>>
    {
        FsExecutionBinding::new(parent.manager()).submit_write_async(provider, request, effects)
    }

    /// Submits a managed copy under the supplied parent operation scope.
    ///
    /// Uses the explicit provider for managed preparation and provider-specific
    /// preflight; this facade does not use its own SPI or cached properties.
    /// The parent manager decides admission and owns the child. A parent stop
    /// propagates to the child, without proving provider effects were rolled
    /// back.
    ///
    /// # Parameters
    /// - `parent`: Open operation scope that owns this child submission.
    /// - `provider`: Provider that prepares the owned copy request.
    /// - `request`: Owned copy parameters.
    /// - `effects`: Bounded pool used to reserve final effect evidence.
    ///
    /// # Returns
    /// A managed copy operation submitted under `parent`.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` with the original request when provider
    /// preparation or manager admission rejects the child, including when
    /// the parent is closed.
    #[inline]
    pub fn submit_copy_in(
        &self,
        parent: &OperationScope,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedCopyRequest,
        effects: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::copy::CopyOutcome>, crate::FsSubmitFailure<FsOwnedCopyRequest>> {
        FsExecutionBinding::new(parent.manager()).submit_copy_async(provider, request, effects)
    }

    /// Submits a managed rename under the supplied parent operation scope.
    ///
    /// Uses the explicit provider for managed preparation and provider-specific
    /// preflight; this facade does not use its own SPI or cached properties.
    /// The parent manager decides admission and owns the child. A parent stop
    /// propagates to the child, without proving provider effects were rolled
    /// back.
    ///
    /// # Parameters
    /// - `parent`: Open operation scope that owns this child submission.
    /// - `provider`: Provider that prepares the owned rename request.
    /// - `request`: Owned rename parameters.
    /// - `effects`: Bounded pool used to reserve final effect evidence.
    ///
    /// # Returns
    /// A managed rename operation submitted under `parent`.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` with the original request when provider
    /// preparation or manager admission rejects the child, including when
    /// the parent is closed.
    #[inline]
    pub fn submit_rename_in(
        &self,
        parent: &OperationScope,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedRenameRequest,
        effects: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::rename::RenameOutcome>, crate::FsSubmitFailure<FsOwnedRenameRequest>>
    {
        FsExecutionBinding::new(parent.manager()).submit_rename_async(provider, request, effects)
    }

    /// Submits staged-tree publication under the supplied parent scope.
    ///
    /// Uses the explicit provider for managed preparation and provider-specific
    /// preflight; this facade does not use its own SPI or cached properties.
    /// The parent manager decides admission and owns the child. A parent stop
    /// propagates to the child, without proving provider effects were rolled
    /// back.
    ///
    /// # Parameters
    /// - `parent`: Open operation scope that owns this child submission.
    /// - `provider`: Provider that prepares the tree publication request.
    /// - `request`: Owned tree publication parameters.
    /// - `effects`: Bounded pool used to reserve final effect evidence.
    ///
    /// # Returns
    /// A managed tree-publication operation submitted under `parent`.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` with the original request when provider
    /// preparation or manager admission rejects the child, including when
    /// the parent is closed.
    #[inline]
    pub fn submit_tree_publish_in(
        &self,
        parent: &OperationScope,
        provider: &dyn ManagedFsTreeSpi,
        request: FsOwnedTreePublishRequest,
        effects: &FsEffectSlotPool,
    ) -> Result<
        AsyncFsMutationOperation<crate::directory::TreePublishOutcome>,
        crate::FsSubmitFailure<FsOwnedTreePublishRequest>,
    > {
        FsExecutionBinding::new(parent.manager()).submit_tree_publish_async(provider, request, effects)
    }

    /// Returns the immutable property snapshot without provider I/O.
    /// # Returns
    /// The immutable provider property snapshot cached by this facade.
    #[inline]
    #[must_use]
    pub fn properties(&self) -> &FileSystemProperties {
        self.core.properties()
    }

    /// Validates a write request without opening a provider session.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Write behavior and guarantees to validate before provider
    ///   work begins.
    /// # Errors
    /// Returns an options, capability, or provider-limit validation error.
    #[inline]
    pub fn validate_write(&self, path: &Path, options: &WriteOptions) -> FsResult<()> {
        self.core.validate_write_request(path, options)
    }

    /// Assesses copy routes without performing provider I/O.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Copy route, conflict, consistency, and metadata guarantees
    ///   requested by the caller.
    ///
    /// # Returns
    /// The available copy assessment for the paths and requested guarantees.
    /// # Errors
    /// Returns an invalid-options or requirement error when no provider or
    /// stream execution route can satisfy the request.
    pub fn assess_copy(&self, source: &Path, target: &Path, options: &CopyOptions) -> FsResult<CopyAssessment> {
        self.core.assess_copy(source, target, options)
    }

    /// Asynchronously reads metadata after local validation completes.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    ///
    /// # Returns
    /// Metadata for `path` returned by the provider.
    /// # Errors
    /// Returns a validation, provider, or provider-contract error when the
    /// path cannot be read or the response identifies another path.
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
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    ///
    /// # Returns
    /// `true` when `path` exists; `false` only for an explicit `NotFound`
    /// result.
    /// # Errors
    /// Returns the original filesystem error for failures other than an
    /// explicit `NotFound` response.
    pub async fn exists(&self, path: &Path) -> FsResult<bool> {
        match self.stat(path).await {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == FsErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.with_operation(FsOperation::Exists)),
        }
    }

    /// Asynchronously opens a validated directory stream.
    ///
    /// # Parameters
    ///
    /// - `scope`: Requested directory or namespace listing scope.
    /// - `options`: Selection, recursion, page-size, and deadline settings for
    ///   the listing.
    ///
    /// # Returns
    /// A validated asynchronous stream for the requested listing.
    /// # Errors
    /// Returns an invalid-options, missing-capability, or provider error when
    /// the scope or options cannot be served.
    #[inline]
    pub async fn list(&self, scope: &ListScope, options: ListOptions) -> FsResult<AsyncDirectoryStream> {
        AsyncDirectoryOperation::new(self).list(scope, options).await
    }

    /// Asynchronously opens a validated reader and verifies its identity.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Read behavior and limits for opening or consuming the
    ///   resource.
    ///
    /// # Returns
    /// A validated asynchronous reader for `path`.
    /// # Errors
    /// Returns a validation, capability, provider, or provider-contract error
    /// when the reader cannot be opened safely.
    #[inline]
    pub async fn open_reader(&self, path: &Path, options: ReadOptions) -> FsResult<AsyncFileReader> {
        self.open_reader_resolved(path, ResolvedReadOptions::new(options)).await
    }

    /// Opens a complete file for explicit-offset reads on one opened object.
    /// The observed length is an upper bound, not a content snapshot.
    ///
    /// # Parameters
    /// - `path`: Logical file path to open.
    /// - `options`: Read options. Only no offset or offset zero, no length, and
    ///   no checksum validation are accepted; version conditions are kept.
    ///
    /// # Returns
    /// A handle whose reads use the object opened by the provider.
    ///
    /// # Errors
    /// Returns path/option errors, missing read or operation errors, the
    /// provider open error, or a provider-contract error for mismatched
    /// identity, missing metadata, non-file kind, or unknown length.
    pub async fn open_random_access_reader(
        &self,
        path: &Path,
        options: ReadOptions,
    ) -> FsResult<AsyncRandomAccessReader> {
        self.validate_path(path, FsOperation::OpenReader)?;
        RandomAccessReadPolicy::validate_open(&options).map_err(|e| self.enrich(e, path, FsOperation::OpenReader))?;
        RandomAccessReadPolicy::validate_capabilities(&options, self.properties().capabilities())
            .map_err(|e| self.enrich(e, path, FsOperation::OpenReader))?;
        self.require(FileSystemCapability::Read, FsOperation::OpenReader, path)?;
        if !self.core.provider_supports(ProviderOperation::OpenRandomAccessReader) {
            return Err(self.enrich(
                FsError::new(
                    FsErrorKind::UnsupportedOperation,
                    FsOperation::OpenReader,
                    "provider does not implement random-access sessions",
                ),
                path,
                FsOperation::OpenReader,
            ));
        }
        let opened = self
            .spi
            .open_random_access_reader(OpenReaderRequest::new(path, ResolvedReadOptions::new(options)))
            .await
            .map_err(|e| self.enrich(e, path, FsOperation::OpenReader))?;
        let (info, session) = opened.into_parts();
        self.validate_opened_info(&info, path)?;
        let len = RandomAccessReadPolicy::opened_len(info.metadata())
            .map_err(|e| self.enrich(e, path, FsOperation::OpenReader))?;
        Ok(AsyncRandomAccessReader::new(
            info,
            len,
            self.properties().limits().max_read_range_bytes(),
            self.properties().info().provider_id().to_owned(),
            session,
        ))
    }

    /// Asynchronously reads an entire file while enforcing a strict byte cap.
    ///
    /// The cap applies to bytes actually read, even when opened metadata
    /// overestimates the resource length.
    ///
    /// Long aggregate reads may periodically return
    /// [`std::task::Poll::Pending`] even when the provider remains ready.
    /// This gives the executor an opportunity to schedule other tasks and
    /// observe cancellation. It does not bound the time spent in a single
    /// provider poll or guarantee a scheduling order; cancellation cannot
    /// interrupt a provider poll already in progress. The byte cap bounds
    /// the result size, not elapsed time.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Read behavior and limits for opening or consuming the
    ///   resource.
    /// - `max_bytes`: Maximum number of bytes this operation may return or
    ///   consume.
    ///
    /// # Returns
    /// The file contents, bounded by `max_bytes`.
    /// # Errors
    /// Returns the reader or read error when validation, opening, or bounded
    /// reading fails.
    #[inline]
    pub async fn read_all(&self, path: &Path, options: ReadOptions, max_bytes: usize) -> FsResult<Vec<u8>> {
        AsyncReadOperation::new(self).read_all(path, options, max_bytes).await
    }

    /// Reads a complete file and reports actual provider open/read polling.
    ///
    /// Local path, option, range, and capability validation completes before
    /// the observer is asked to allow an open. Provider `Pending` and `Ready`
    /// events describe polls of SPI open/read futures only. An opened envelope
    /// is validated after the open `Ready` event; aggregate fairness yields
    /// do not generate events.
    ///
    /// # Type Parameters
    /// - `O`: Observer that controls and records provider boundaries.
    ///
    /// # Parameters
    /// - `path`: File to read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum accepted byte count.
    /// - `observer`: Boundary callback and provider-poll event receiver.
    ///
    /// # Returns
    /// The complete contents when they fit within `max_bytes`.
    ///
    /// # Errors
    /// Returns [`ObservedReadError::Stopped`] when the observer stops before a
    /// provider call, or [`ObservedReadError::Provider`] for validation,
    /// provider, or buffer errors.
    pub async fn read_all_observed<O: ReadIoObserver>(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
        observer: &mut O,
    ) -> Result<Vec<u8>, ObservedReadError> {
        AsyncReadOperation::new(self)
            .read_all_observed(path, options, max_bytes, observer)
            .await
    }

    /// Asynchronously reads at most max_bytes from a file.
    ///
    /// Long aggregate reads may periodically return
    /// [`std::task::Poll::Pending`] even when the provider remains ready.
    /// This gives the executor an opportunity to schedule other tasks and
    /// observe cancellation. It does not bound the time spent in a single
    /// provider poll or guarantee a scheduling order; cancellation cannot
    /// interrupt a provider poll already in progress. The byte cap bounds
    /// the result size, not elapsed time.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Read behavior and limits for opening or consuming the
    ///   resource.
    /// - `max_bytes`: Maximum number of bytes this operation may return or
    ///   consume.
    ///
    /// # Returns
    /// The bounded prefix and read outcome reported by the operation.
    /// # Errors
    /// Returns the reader or read error when validation, opening, or bounded
    /// reading fails.
    #[inline]
    pub async fn read_prefix(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<crate::read::PrefixReadOutcome> {
        AsyncReadOperation::new(self)
            .read_prefix(path, options, max_bytes)
            .await
    }

    /// Reads a file prefix and reports actual provider open/read polling.
    ///
    /// The observer may stop the operation before any provider open or read
    /// call. Provider `Pending` and `Ready` events describe polls of those
    /// futures only; aggregate fairness yields do not generate events.
    ///
    /// # Type Parameters
    /// - `O`: Observer that controls and records provider boundaries.
    ///
    /// # Parameters
    /// - `path`: File whose prefix is read.
    /// - `options`: Options used to open the reader.
    /// - `max_bytes`: Maximum number of prefix bytes to accumulate.
    /// - `observer`: Boundary callback and provider-poll event receiver.
    ///
    /// # Returns
    /// The bounded prefix with its termination reason and open-time metadata.
    ///
    /// # Errors
    /// Returns [`ObservedReadError::Stopped`] when the observer stops after
    /// local preflight and before a provider call, or
    /// [`ObservedReadError::Provider`] when local or envelope validation,
    /// opening, reading, or buffering fails.
    pub async fn read_prefix_observed<O: ReadIoObserver>(
        &self,
        path: &Path,
        options: ReadOptions,
        max_bytes: usize,
        observer: &mut O,
    ) -> Result<crate::read::PrefixReadOutcome, ObservedReadError> {
        AsyncReadOperation::new(self)
            .read_prefix_observed(path, options, max_bytes, observer)
            .await
    }

    /// Asynchronously opens a validated writer and verifies its identity.
    ///
    /// An [`OpenFailure`] identifies where the attempt stopped:
    /// [`OpenFailureStage::Preflight`] means local validation rejected it
    /// before provider I/O; [`OpenFailureStage::ProviderOpen`] means the
    /// provider returned an error before an opened envelope; and
    /// [`OpenFailureStage::OutcomeValidation`] means an opened envelope failed
    /// identity validation. The last stage can retain a [`RejectedAsyncWriter`]
    /// that owns an isolated session with abort-only cleanup authority. Handle
    /// that recovery explicitly with `abort_async`; dropping the failure or
    /// receiving no recovery does not prove that no side effect occurred.
    /// Keep the typed failure intact when recovery matters instead of using `?`
    /// to discard it.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Write behavior and guarantees to validate before provider
    ///   work begins.
    ///
    /// # Returns
    /// A validated writer, or a typed failure retaining any isolated recovery
    /// session.
    /// # Errors
    /// Returns the contextual error, opening stage, and any isolated recovery
    /// session when validation, provider opening, or outcome validation fails.
    pub async fn open_writer(
        &self,
        path: &Path,
        options: WriteOptions,
    ) -> Result<AsyncFileWriter, OpenFailure<RejectedAsyncWriter>> {
        self.core
            .validate_write_request(path, &options)
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        let atomicity = options.atomicity();
        let durability = options.durability();
        let opened = self
            .spi
            .open_writer(OpenWriterRequest::new(path, ResolvedWriteOptions::new(options)))
            .await
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
                Some(RejectedAsyncWriter::new(
                    session,
                    self.properties().info().provider_id(),
                    Some(path.clone()),
                )),
            ));
        }
        Ok(AsyncFileWriter::new(
            info,
            session,
            atomicity,
            durability,
            self.properties().info().provider_id(),
            self.properties().limits().max_write_bytes().maximum(),
        ))
    }

    /// Begins an owning asynchronous whole-file write operation.
    ///
    /// Moves `path`, `bytes`, and `options` into the request and retains a
    /// clone of this filesystem. Construction performs no provider I/O.
    /// Keep the operation outside the cancellation scope and cancel only
    /// its `execute` future so publication facts and recovery
    /// responsibility remain available.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `bytes`: Payload to write as one complete resource.
    /// - `options`: Write behavior and guarantees to validate before provider
    ///   work begins.
    ///
    /// # Returns
    ///
    /// A ready operation that no longer borrows the caller's filesystem or
    /// payload. Successful execution releases the payload allocation. Failure
    /// and cancellation retain the suffix after confirmed writes for
    /// inspection; that suffix is safe to retry only when provider-specific
    /// recovery establishes that replay cannot duplicate publication.
    ///
    /// # Errors
    ///
    /// Returns a `NotPublished` failure with zero confirmed bytes if the path,
    /// capabilities, options, or payload size fail local validation. Rejected
    /// construction consumes and releases the payload.
    pub fn begin_write_all(
        &self,
        path: Path,
        bytes: Vec<u8>,
        options: WriteOptions,
    ) -> Result<AsyncWriteAllOperation, AsyncWriteAllOperationFailure> {
        self.core.validate_write_request(&path, &options).map_err(|error| {
            AsyncWriteAllOperationFailure::new(error, crate::write::WriteFailureState::NotPublished, 0)
        })?;
        self.properties()
            .limits()
            .validate_write_size(&path, bytes.len())
            .map_err(|error| {
                AsyncWriteAllOperationFailure::new(
                    self.core.enrich(error, Some(&path), FsOperation::Write),
                    crate::write::WriteFailureState::NotPublished,
                    0,
                )
            })?;
        Ok(AsyncWriteAllOperation::new(self.clone(), path, bytes, options))
    }

    /// Asynchronously creates a directory after local validation.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Options that define the operation and its requested
    ///   guarantees.
    ///
    /// # Returns
    /// The provider-confirmed directory creation outcome.
    /// # Errors
    /// Returns a validation, capability, provider, or provider-contract error
    /// when the directory cannot be created as requested.
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
            return Err(self.core.contract_error(
                path,
                FsOperation::CreateDir,
                "provider accepted an existing directory without exists_ok",
            ));
        }
        Ok(outcome)
    }

    /// Asynchronously deletes a file after local validation.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Deletion behavior, including whether a missing target is
    ///   acceptable.
    ///
    /// # Returns
    /// The provider-confirmed file deletion outcome.
    /// # Errors
    /// Returns a validation, capability, provider, or provider-contract error
    /// when deletion cannot be confirmed.
    #[inline]
    pub async fn delete_file(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, false).await
    }

    /// Asynchronously deletes a directory after local validation.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Deletion behavior, including whether a missing target is
    ///   acceptable.
    ///
    /// # Returns
    /// The provider-confirmed directory deletion outcome.
    /// # Errors
    /// Returns a validation, capability, provider, or provider-contract error
    /// when deletion cannot be confirmed.
    #[inline]
    pub async fn delete_directory(&self, path: &Path, options: DeleteOptions) -> FsResult<DeleteOutcome> {
        self.delete(path, options, true).await
    }

    /// Renames one resource through the single asynchronous provider primitive.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Overwrite, atomicity, and durability requirements for the
    ///   rename.
    ///
    /// # Returns
    /// The provider-confirmed rename outcome.
    /// # Errors
    /// Returns [`RenameFailure`] with the confirmed transition state when
    /// validation, provider execution, or outcome validation fails.
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
    ///
    /// # Parameters
    ///
    /// - `options`: Parent, naming, and parent-creation settings for the
    ///   temporary resource.
    ///
    /// # Returns
    /// A validated temporary file, or a typed failure retaining cleanup
    /// authority.
    /// # Errors
    /// Returns [`OpenFailure`] when preflight, provider creation, or temporary
    /// identity validation fails.
    pub async fn create_temp_file(
        &self,
        options: TempOptions,
    ) -> Result<AsyncTempFile, OpenFailure<RejectedAsyncTempResource>> {
        let parent = options.parent().cloned();
        self.core
            .validate_temp_parent(parent.as_ref())
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        self.core
            .require(FileSystemCapability::TempFile, FsOperation::CreateTemp, parent.as_ref())
            .map_err(|error| OpenFailure::new(error, OpenFailureStage::Preflight, None))?;
        let opened = self
            .spi
            .create_temp_file(crate::spi::CreateTempFileRequest::new(options))
            .await
            .map_err(|error| {
                OpenFailure::new(
                    self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp),
                    OpenFailureStage::ProviderOpen,
                    None,
                )
            })?;
        let (info, session) = opened.into_parts();
        if let Err(cause) = self.validate_temp_info(&info, crate::metadata::FileKind::File, parent.as_ref()) {
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
                Some(RejectedAsyncTempResource::new(
                    session,
                    self.properties().info().provider_id(),
                    parent,
                )),
            ));
        }
        Ok(AsyncTempFile::new(
            self.clone(),
            info.path().clone(),
            session,
            "temporary file",
        ))
    }

    /// Asynchronously creates a temporary directory and validates its identity.
    ///
    /// # Parameters
    ///
    /// - `options`: Parent, naming, and parent-creation settings for the
    ///   temporary resource.
    ///
    /// # Returns
    /// A validated temporary directory, or a typed failure retaining cleanup
    /// authority.
    /// # Errors
    /// Returns [`OpenFailure`] when preflight, provider creation, or temporary
    /// identity validation fails.
    pub async fn create_temp_directory(
        &self,
        options: TempOptions,
    ) -> Result<AsyncTempDirectory, OpenFailure<RejectedAsyncTempResource>> {
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
            .create_temp_directory(crate::spi::CreateTempDirectoryRequest::new(options))
            .await
            .map_err(|error| {
                OpenFailure::new(
                    self.core.enrich(error, parent.as_ref(), FsOperation::CreateTemp),
                    OpenFailureStage::ProviderOpen,
                    None,
                )
            })?;
        let (info, session) = opened.into_parts();
        if let Err(cause) = self.validate_temp_info(&info, crate::metadata::FileKind::Directory, parent.as_ref()) {
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
                Some(RejectedAsyncTempResource::new(
                    session,
                    self.properties().info().provider_id(),
                    parent,
                )),
            ));
        }
        Ok(AsyncTempDirectory::new(self.clone(), info.path().clone(), session))
    }

    /// Begins a copy after synchronous path, option, and capability preflight.
    ///
    /// This method performs no provider I/O. Provider work begins only when
    /// [`AsyncCopyOperation::execute`] is polled.
    /// Keep the returned operation outside the cancellation scope and cancel
    /// only its `execute` future. The operation then remains available to
    /// inspect publication state and take any retained writer recovery.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Copy route, conflict, consistency, and metadata guarantees
    ///   requested by the caller.
    ///
    /// # Returns
    /// A ready copy operation whose execution has not started.
    /// # Errors
    /// Returns [`AsyncCopyFailure`] when local preflight rejects the request.
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

    /// Performs all no-I/O copy validation required before an operation exists.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Copy route, conflict, consistency, and metadata guarantees
    ///   requested by the caller.
    /// # Errors
    /// Returns a path, options, or capability validation error.
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
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Overwrite, atomicity, and durability requirements for the
    ///   rename.
    /// # Errors
    /// Returns a path, options, or capability validation error.
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
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `options`: Deletion behavior, including whether a missing target is
    ///   acceptable.
    /// - `directory`: Whether the delete request targets a directory.
    /// # Returns
    /// The provider-confirmed deletion outcome.
    /// # Errors
    /// Returns a path, options, capability, provider, or contract error.
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
            return Err(self.core.contract_error(
                path,
                FsOperation::Delete,
                "provider accepted a missing target without missing_ok",
            ));
        }
        Ok(outcome)
    }

    /// Validates a logical path against the cached provider snapshot.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// # Errors
    /// Returns a path validation error.
    #[inline]
    fn validate_path(&self, path: &Path, operation: FsOperation) -> FsResult<()> {
        self.core.validate_path(path, operation)
    }

    /// Adds source, target, and provider facts to an asynchronous rename
    /// failure.
    ///
    /// # Parameters
    ///
    /// - `error`: Filesystem error whose kind, source, and established facts
    ///   are retained.
    /// - `state`: Operation state used to classify confirmed side effects.
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// # Returns
    /// A rename failure enriched with operation, source, target, and provider
    /// context.
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
    ///
    /// # Parameters
    ///
    /// - `error`: Filesystem error whose kind, source, and established facts
    ///   are retained.
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// # Returns
    /// The error with missing path, operation, and provider context added.
    #[inline]
    fn enrich(&self, error: FsError, path: &Path, operation: FsOperation) -> FsError {
        self.core.enrich(error, Some(path), operation)
    }

    /// Creates a provider-contract violation bound to the requested path.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `message`: Diagnostic message for the constructed filesystem error.
    /// # Returns
    /// A provider-contract violation associated with `path`.
    #[inline]
    fn contract_error(&self, path: &Path, message: &'static str) -> FsError {
        self.core
            .contract_error(path, FsOperation::ValidateProviderOutcome, message)
    }

    /// Validates a provider-opened handle identity before exposing it to
    /// callers.
    ///
    /// # Parameters
    ///
    /// - `info`: Opened-resource identity and metadata returned by the
    ///   provider.
    /// - `path`: Logical path of the resource involved in this operation.
    /// # Errors
    /// Returns a provider-contract error when the opened identity does not
    /// match `path`.
    #[inline]
    fn validate_opened_info(&self, info: &crate::metadata::OpenedFileInfo, path: &Path) -> FsResult<()> {
        self.core.validate_opened_info(info, path)
    }

    /// Validates a provider-created temporary resource before exposing it.
    ///
    /// # Parameters
    ///
    /// - `info`: Opened-resource identity and metadata returned by the
    ///   provider.
    /// - `expected_kind`: Resource kind required for the opened object.
    /// - `requested_parent`: Optional parent location supplied for temporary
    ///   creation.
    /// # Errors
    /// Returns a provider-contract error when the temporary identity is
    /// invalid.
    #[inline]
    fn validate_temp_info(
        &self,
        info: &crate::metadata::OpenedFileInfo,
        expected_kind: crate::metadata::FileKind,
        requested_parent: Option<&Path>,
    ) -> FsResult<()> {
        self.core.validate_temp_info(info, expected_kind, requested_parent)
    }
}

impl AsyncFileSystem {
    /// Returns shared deterministic facade policy to operation objects.
    /// # Returns
    /// The shared immutable facade core.
    #[inline]
    #[must_use]
    pub(crate) fn core(&self) -> &FacadeCore {
        &self.core
    }

    /// Returns the asynchronous provider implementation to operation objects.
    /// # Returns
    /// The asynchronous provider implementation backing this facade.
    #[inline]
    #[must_use]
    pub(crate) fn spi(&self) -> &dyn AsyncFileSystemSpi {
        self.spi.as_ref()
    }

    /// Opens a facade-validated asynchronous reader with an internal hint.
    ///
    /// # Parameters
    ///
    /// - `path`: Logical path of the resource involved in this operation.
    /// - `resolved`: Validated read options and the facade’s resolved
    ///   read-planning hints.
    /// # Returns
    /// A validated reader configured with resolved read options.
    /// # Errors
    /// Returns a validation, capability, provider, or provider-contract error.
    pub(crate) async fn open_reader_resolved(
        &self,
        path: &Path,
        resolved: ResolvedReadOptions,
    ) -> FsResult<AsyncFileReader> {
        let resolved = self.prepare_open_reader(path, resolved)?;
        let opened = self.dispatch_open_reader(path, resolved).await?;
        self.validate_opened_reader(path, opened)
    }

    /// Performs local validation before an observed or ordinary SPI open.
    pub(crate) fn prepare_open_reader(
        &self,
        path: &Path,
        resolved: ResolvedReadOptions,
    ) -> FsResult<ResolvedReadOptions> {
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
        Ok(resolved)
    }

    /// Dispatches an already-prepared open to the provider.
    pub(crate) async fn dispatch_open_reader(
        &self,
        path: &Path,
        resolved: ResolvedReadOptions,
    ) -> FsResult<OpenedAsyncReader> {
        let opened = self.spi
            .open_reader(OpenReaderRequest::new(path, resolved))
            .await
            .map_err(|error| self.enrich(error, path, FsOperation::OpenReader))?;
        Ok(opened)
    }

    /// Validates provider-returned identity and constructs the reader.
    pub(crate) fn validate_opened_reader(
        &self,
        path: &Path,
        opened: OpenedAsyncReader,
    ) -> FsResult<AsyncFileReader> {
        self.validate_opened_info(opened.info(), path)?;
        Ok(opened.into_reader())
    }

    /// Rechecks provider-reported success against requested copy guarantees.
    ///
    /// # Parameters
    ///
    /// - `outcome`: Provider-reported copy outcome to validate against the
    ///   request.
    /// - `options`: Copy route, conflict, consistency, and metadata guarantees
    ///   requested by the caller.
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// # Returns
    /// The outcome when it satisfies the requested copy guarantees.
    /// # Errors
    /// Returns a contextual copy failure when provider-reported facts violate
    /// those guarantees.
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
                outcome.violation_failure_state(options),
                *outcome.stats(),
                source,
                target,
            ));
        }
        Ok(outcome)
    }

    /// Requires a provider-advertised capability before operation creation.
    ///
    /// # Parameters
    ///
    /// - `capability`: Capability required before provider work can begin.
    /// - `operation`: Filesystem operation used for validation or error
    ///   context.
    /// - `path`: Logical path of the resource involved in this operation.
    /// # Errors
    /// Returns an error when the provider does not advertise `capability`.
    #[inline]
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
    ///
    /// # Parameters
    ///
    /// - `error`: Filesystem error whose kind, source, and established facts
    ///   are retained.
    /// - `state`: Operation state used to classify confirmed side effects.
    /// - `stats`: Copy progress statistics reported by the operation.
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// # Returns
    /// A copy failure enriched with operation, source, target, provider, and
    /// progress context.
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

    /// Performs no-I/O preflight for asynchronous temporary persistence.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// - `options`: Options that define the operation and its requested
    ///   guarantees.
    /// # Errors
    /// Returns a path, options, or capability validation error.
    pub(crate) fn preflight_temp_persist(
        &self,
        source: &Path,
        target: &Path,
        options: &PersistOptions,
    ) -> FsResult<()> {
        self.validate_path(source, FsOperation::PersistTemp).map_err(|error| {
            self.enrich(error, source, FsOperation::PersistTemp)
                .with_failure_path(source.clone())
                .with_path(source.clone())
                .with_target(target.clone())
        })?;
        self.validate_path(target, FsOperation::PersistTemp).map_err(|error| {
            self.enrich(error, source, FsOperation::PersistTemp)
                .with_failure_target(target.clone())
                .with_path(source.clone())
                .with_target(target.clone())
        })?;
        options
            .validate_against(self.properties().capabilities())
            .map_err(|error| {
                self.enrich(error, source, FsOperation::PersistTemp)
                    .with_target(target.clone())
            })
    }

    /// Validates a provider-generated target reported by temporary keep.
    ///
    /// # Parameters
    ///
    /// - `source`: Logical path of the source resource.
    /// - `target`: Destination logical path affected by the operation.
    /// # Errors
    /// Returns a provider-contract error when the generated target is not a
    /// valid logical path.
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
