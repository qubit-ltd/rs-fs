// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Controllable asynchronous SPI used only by facade-level behavior tests.

use std::io::{
    Error as IoError,
    Result as IoResult,
};
use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};
use std::task::{
    Context,
    Poll,
};

use qubit_fs::spi::{
    AsyncDirectoryStreamSession,
    AsyncFileSystemSpi,
    AsyncFileWriteSession,
    AsyncTempResourceSpi,
    CopyAttempt,
    CopyDeclineReason,
    CopyRequest,
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedAsyncDirectoryStream,
    OpenedAsyncReader,
    OpenedAsyncTempDirectory,
    OpenedAsyncTempFile,
    OpenedAsyncWriter,
    RenameRequest,
    SpiCopyFailure,
    SpiFuture,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    AchievedAtomicity,
    AsyncFileSystem,
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileKind,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    OpenedFileInfo,
    Path,
    PathConstraints,
    PathSemantics,
    PersistOutcome,
    PublicationMethod,
    RenameFailureState,
    RenameOutcome,
    WriteFailure,
    WriteFailureState,
    WriteOutcome,
};
use qubit_io::{
    AsyncInput,
    AsyncOutput,
};

/// A fallback await point that can remain pending or return a deterministic
/// failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AsyncCopyStage {
    Stat,
    OpenReader,
    OpenWriter,
    TryCopy,
    ReaderRead,
    WriterWrite,
    WriterFlush,
    WriterCommit,
}

/// Controls the recording provider's externally observable behavior.
#[derive(Clone, Debug, Default)]
pub(crate) struct AsyncRecordingConfig {
    pub(crate) pending_stage: Option<AsyncCopyStage>,
    pub(crate) failing_stage: Option<AsyncCopyStage>,
    pub(crate) invalid_temp_identity: bool,
    pub(crate) atomic_temp_persist: bool,
    pub(crate) completed_copy: Option<AchievedAtomicity>,
    pub(crate) server_side_copy: bool,
    pub(crate) copy_failure: bool,
    pub(crate) rename_atomicity: Option<AchievedAtomicity>,
    pub(crate) temp_persist_indeterminate: bool,
    pub(crate) temp_cleanup_failure: bool,
    pub(crate) writer_atomicity: Option<AchievedAtomicity>,
    pub(crate) writer_commit_failure: Option<WriteFailureState>,
    pub(crate) temp_persist_atomicity: Option<AchievedAtomicity>,
    pub(crate) directory_entries: Vec<qubit_fs::DirEntry>,
    pub(crate) directory_error: bool,
}

/// Exposes ordered provider call facts without leaking session internals.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct AsyncRecordingProbe(
    Arc<Mutex<Vec<&'static str>>>,
    Arc<Mutex<usize>>,
);
impl AsyncRecordingProbe {
    /// Returns the calls observed so far.
    pub(crate) fn calls(&self) -> Vec<&'static str> {
        self.0.lock().expect("calls lock should succeed").clone()
    }
    /// Returns local writer cancellation notifications.
    #[allow(dead_code)]
    pub(crate) fn writer_cancellations(&self) -> usize {
        *self.1.lock().expect("cancellation lock should succeed")
    }
}

/// Creates an async facade with controllable fallback and temporary sessions.
pub(crate) fn async_recording_file_system(
    config: AsyncRecordingConfig,
) -> (AsyncFileSystem, AsyncRecordingProbe) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let cancellations = Arc::new(Mutex::new(0));
    let probe =
        AsyncRecordingProbe(Arc::clone(&calls), Arc::clone(&cancellations));
    let file_system = AsyncFileSystem::from_spi(AsyncRecordingSpi {
        config,
        calls,
        cancellations,
    })
    .expect("recording async facade should construct");
    (file_system, probe)
}

/// Records facade calls and supplies predictable fallback handles.
struct AsyncRecordingSpi {
    config: AsyncRecordingConfig,
    calls: Arc<Mutex<Vec<&'static str>>>,
    cancellations: Arc<Mutex<usize>>,
}
impl AsyncRecordingSpi {
    /// Records an SPI invocation.
    fn record(&self, call: &'static str) {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push(call);
    }
    /// Builds the property snapshot used by tests.
    fn properties_for(&self) -> FileSystemProperties {
        let mut capabilities = FileSystemCapabilities::new()
            .with(FileSystemCapability::Copy)
            .with(FileSystemCapability::Read)
            .with(FileSystemCapability::Write)
            .with(FileSystemCapability::TempFile)
            .with(FileSystemCapability::TempDirectory)
            .with(FileSystemCapability::List);
        if self.config.completed_copy.is_some() {
            capabilities = capabilities
                .with(FileSystemCapability::AtomicReplace)
                .with(FileSystemCapability::DurableCopy);
        }
        if self.config.server_side_copy {
            capabilities =
                capabilities.with(FileSystemCapability::ServerSideCopy);
        }
        if self.config.rename_atomicity.is_some() {
            capabilities = capabilities
                .with(FileSystemCapability::Rename)
                .with(FileSystemCapability::AtomicRename);
        }
        if self.config.atomic_temp_persist {
            capabilities =
                capabilities.with(FileSystemCapability::AtomicTempPersist);
        }
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-recording")
                    .expect("test id should be valid"),
                "async-recording",
                PathSemantics::Hierarchical,
            ),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
        )
        .expect("test properties should be valid")
    }
    /// Returns an opened identity for one requested path.
    fn info(path: &Path) -> OpenedFileInfo {
        OpenedFileInfo::new(
            FileSystemId::new("async-recording")
                .expect("test id should be valid"),
            path.clone(),
        )
    }
    /// Returns a temporary identity, optionally invalid for boundary testing.
    fn temp_info(&self) -> OpenedFileInfo {
        let id = if self.config.invalid_temp_identity {
            FileSystemId::new("other-provider")
                .expect("test id should be valid")
        } else {
            FileSystemId::new("async-recording")
                .expect("test id should be valid")
        };
        OpenedFileInfo::new(
            id,
            Path::parse("/tmp/recording").expect("test path should parse"),
        )
    }
}
impl AsyncFileSystemSpi for AsyncRecordingSpi {
    fn properties(&self) -> FileSystemProperties {
        self.properties_for()
    }
    fn stat<'a>(
        &'a self,
        request: StatRequest<'a>,
    ) -> SpiFuture<'a, FsResult<StatResponse>> {
        self.record("stat");
        if self.config.pending_stage == Some(AsyncCopyStage::Stat) {
            return Box::pin(std::future::pending());
        }
        if self.config.failing_stage == Some(AsyncCopyStage::Stat) {
            return Box::pin(async { Err(unused()) });
        }
        let mut metadata = FileMetadata::new(FileKind::File);
        metadata.len = Some(5);
        Box::pin(async move {
            Ok(StatResponse::new(request.path().clone(), metadata))
        })
    }
    fn list<'a>(
        &'a self,
        _: ListRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncDirectoryStream>> {
        let entries = self.config.directory_entries.clone();
        let fail = self.config.directory_error;
        Box::pin(async move {
            Ok(OpenedAsyncDirectoryStream::new(Box::new(
                RecordingDirectorySession { entries, fail },
            )))
        })
    }
    fn open_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        self.record("open_reader");
        if self.config.pending_stage == Some(AsyncCopyStage::OpenReader) {
            return Box::pin(std::future::pending());
        }
        if self.config.failing_stage == Some(AsyncCopyStage::OpenReader) {
            return Box::pin(async { Err(unused()) });
        }
        let info = Self::info(request.path());
        let config = self.config.clone();
        Box::pin(async move {
            Ok(OpenedAsyncReader::new(
                info,
                Box::new(RecordingInput {
                    position: 0,
                    config,
                }),
            ))
        })
    }
    fn open_writer<'a>(
        &'a self,
        request: OpenWriterRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
        self.record("open_writer");
        if self.config.pending_stage == Some(AsyncCopyStage::OpenWriter) {
            return Box::pin(std::future::pending());
        }
        if self.config.failing_stage == Some(AsyncCopyStage::OpenWriter) {
            return Box::pin(async { Err(unused()) });
        }
        let info = Self::info(request.path());
        let config = self.config.clone();
        Box::pin(async move {
            Ok(OpenedAsyncWriter::new(
                info,
                Box::new(RecordingWriter {
                    config,
                    cancellations: Arc::clone(&self.cancellations),
                }),
            ))
        })
    }
    fn create_directory<'a>(
        &'a self,
        _: CreateDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<CreateDirectoryOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_file<'a>(
        &'a self,
        _: DeleteFileRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn delete_directory<'a>(
        &'a self,
        _: DeleteDirectoryRequest<'a>,
    ) -> SpiFuture<'a, FsResult<DeleteOutcome>> {
        Box::pin(async { Err(unused()) })
    }
    fn try_copy<'a>(
        &'a self,
        _: CopyRequest<'a>,
    ) -> SpiFuture<'a, Result<CopyAttempt, SpiCopyFailure>> {
        self.record("try_copy");
        if self.config.pending_stage == Some(AsyncCopyStage::TryCopy) {
            return Box::pin(std::future::pending());
        }
        if self.config.copy_failure {
            return Box::pin(async {
                Err(SpiCopyFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::Copy,
                        "injected copy failure",
                    ),
                    qubit_fs::CopyFailureState::Indeterminate,
                    qubit_fs::CopyStats::default(),
                ))
            });
        }
        if let Some(atomicity) = self.config.completed_copy {
            return Box::pin(async move {
                Ok(CopyAttempt::Completed(qubit_fs::CopyOutcome::new(
                    qubit_fs::CopyStats::default(),
                    qubit_fs::CopyMethod::Native,
                    atomicity,
                )))
            });
        }
        Box::pin(async {
            Ok(CopyAttempt::Declined(CopyDeclineReason::NotApplicable))
        })
    }
    fn rename<'a>(
        &'a self,
        _: RenameRequest<'a>,
    ) -> SpiFuture<'a, Result<RenameOutcome, SpiRenameFailure>> {
        self.record("rename");
        if let Some(atomicity) = self.config.rename_atomicity {
            return Box::pin(async move {
                Ok(RenameOutcome::new(
                    atomicity,
                    PublicationMethod::AtomicRename,
                ))
            });
        }
        Box::pin(async {
            Err(SpiRenameFailure::new(
                unused(),
                RenameFailureState::Unchanged,
            ))
        })
    }
    fn create_temp_file<'a>(
        &'a self,
        _: CreateTempFileRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        self.record("create_temp_file");
        let info = self.temp_info();
        let calls = Arc::clone(&self.calls);
        Box::pin(async move {
            Ok(OpenedAsyncTempFile::new(
                info,
                Box::new(RecordingTempSession {
                    calls,
                    indeterminate_persist: self
                        .config
                        .temp_persist_indeterminate,
                    cleanup_failure: self.config.temp_cleanup_failure,
                    atomicity: self.config.temp_persist_atomicity,
                }),
            ))
        })
    }
    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        self.record("create_temp_directory");
        let info = self.temp_info();
        let calls = Arc::clone(&self.calls);
        Box::pin(async move {
            Ok(OpenedAsyncTempDirectory::new(
                info,
                Box::new(RecordingTempSession {
                    calls,
                    indeterminate_persist: self
                        .config
                        .temp_persist_atomicity
                        .is_none()
                        && self.config.temp_persist_indeterminate,
                    cleanup_failure: self.config.temp_cleanup_failure,
                    atomicity: self.config.temp_persist_atomicity,
                }),
            ))
        })
    }
}

/// Enumerates configured entries or one configured provider failure.
struct RecordingDirectorySession {
    entries: Vec<qubit_fs::DirEntry>,
    fail: bool,
}
impl AsyncDirectoryStreamSession for RecordingDirectorySession {
    fn next_entry_async(
        &mut self,
    ) -> SpiFuture<'_, FsResult<Option<qubit_fs::DirEntry>>> {
        if self.fail {
            self.fail = false;
            return Box::pin(async { Err(unused()) });
        }
        Box::pin(async move { Ok(self.entries.pop()) })
    }
}

/// Supplies fallback source bytes and one controllable read await point.
struct RecordingInput {
    position: usize,
    config: AsyncRecordingConfig,
}
impl AsyncInput for RecordingInput {
    type Item = u8;
    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        if this.config.pending_stage == Some(AsyncCopyStage::ReaderRead) {
            return Poll::Pending;
        }
        if this.config.failing_stage == Some(AsyncCopyStage::ReaderRead) {
            return Poll::Ready(Err(IoError::other("injected read failure")));
        }
        let bytes = b"bytes";
        let read = bytes[this.position..].len().min(count);
        output[index..index + read]
            .copy_from_slice(&bytes[this.position..this.position + read]);
        this.position += read;
        Poll::Ready(Ok(read))
    }
}

/// Supplies fallback destination I/O and publication behavior.
struct RecordingWriter {
    config: AsyncRecordingConfig,
    cancellations: Arc<Mutex<usize>>,
}
impl AsyncOutput for RecordingWriter {
    type Item = u8;
    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        _: &[u8],
        _: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let config = self.get_mut().config.clone();
        if config.pending_stage == Some(AsyncCopyStage::WriterWrite) {
            Poll::Pending
        } else if config.failing_stage == Some(AsyncCopyStage::WriterWrite) {
            Poll::Ready(Err(IoError::other("injected write failure")))
        } else {
            Poll::Ready(Ok(count))
        }
    }
    fn poll_flush(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<IoResult<()>> {
        let config = self.get_mut().config.clone();
        if config.pending_stage == Some(AsyncCopyStage::WriterFlush) {
            Poll::Pending
        } else if config.failing_stage == Some(AsyncCopyStage::WriterFlush) {
            Poll::Ready(Err(IoError::other("injected flush failure")))
        } else {
            Poll::Ready(Ok(()))
        }
    }
}
impl AsyncFileWriteSession for RecordingWriter {
    fn commit_async<'a>(
        self: Pin<&'a mut Self>,
    ) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
        let config = self.get_mut().config.clone();
        if config.pending_stage == Some(AsyncCopyStage::WriterCommit) {
            return Box::pin(std::future::pending());
        }
        Box::pin(async move {
            if let Some(state) = config.writer_commit_failure {
                return Err(WriteFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::CommitWriter,
                        "injected commit failure",
                    ),
                    state,
                ));
            }
            if config.failing_stage == Some(AsyncCopyStage::WriterCommit) {
                return Err(WriteFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::CommitWriter,
                        "injected commit failure",
                    ),
                    WriteFailureState::NotPublished,
                ));
            }
            Ok(WriteOutcome::new(
                config
                    .writer_atomicity
                    .unwrap_or(AchievedAtomicity::NonAtomic),
                PublicationMethod::StreamCopy,
            ))
        })
    }
    fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

/// Records temporary lifecycle calls and completes them successfully.
struct RecordingTempSession {
    calls: Arc<Mutex<Vec<&'static str>>>,
    indeterminate_persist: bool,
    atomicity: Option<AchievedAtomicity>,
    cleanup_failure: bool,
}
impl RecordingTempSession {
    /// Records one temporary lifecycle call.
    fn record(&self, call: &'static str) {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push(call);
    }
}
impl AsyncTempResourceSpi for RecordingTempSession {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        self.as_ref().get_ref().record("cleanup");
        let cleanup_failure = self.as_ref().get_ref().cleanup_failure;
        Box::pin(async move {
            if cleanup_failure {
                Err(FsError::with_source(
                    FsErrorKind::Io,
                    FsOperation::CleanupTemp,
                    "injected cleanup failure",
                    IoError::other("underlying cleanup failure"),
                ))
            } else {
                Ok(())
            }
        })
    }
    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        self.as_ref().get_ref().record("keep");
        Box::pin(async { Ok(()) })
    }
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: qubit_fs::spi::PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, qubit_fs::spi::SpiPersistFailure>>
    {
        self.as_ref().get_ref().record("persist");
        let target = request.target().clone();
        let indeterminate = self.as_ref().get_ref().indeterminate_persist;
        Box::pin(async move {
            if indeterminate {
                return Err(qubit_fs::spi::SpiPersistFailure::new(
                    FsError::new(
                        FsErrorKind::Indeterminate,
                        FsOperation::PersistTemp,
                        "injected indeterminate persist",
                    ),
                    qubit_fs::PersistFailureState::Indeterminate,
                ));
            }
            Ok(PersistOutcome::new(
                target,
                self.as_ref()
                    .get_ref()
                    .atomicity
                    .unwrap_or(AchievedAtomicity::Atomic),
                PublicationMethod::AtomicRename,
            ))
        })
    }
}

/// Builds an error for unsupported provider methods outside this test scope.
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}
