// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public copy recovery contract against an observable in-memory destination.

use std::io::Error as IoError;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyOptions;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriterRecovery;
use qubit_fs::write::WriterState;
use qubit_io::Input;
use qubit_io::Output;

/// Selects the point at which the scripted source read fails or exceeds a
/// limit.
#[derive(Clone, Copy)]
enum ReadScenario {
    FailAfterPrefix,
    ExceedLimit,
    FailImmediately,
}

/// Chooses whether writes alter the visible destination or remain staged.
#[derive(Clone, Copy)]
enum WriterMode {
    Direct,
    Staging,
}

/// Captures destination creation, published/staged bytes, commits, and aborts.
#[derive(Default)]
struct VisibleTarget {
    created: bool,
    bytes: Vec<u8>,
    staging: Vec<u8>,
    commits: usize,
    aborts: usize,
}

/// Supplies scripted reads and writers that expose publication behavior.
struct CopyFixture {
    state: Arc<Mutex<VisibleTarget>>,
    scenario: ReadScenario,
    mode: WriterMode,
}

/// Parses a path for the public copy contract cases.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Builds provider properties for the fixture's supported read/write
/// operations.
fn properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("copy-contract").expect("valid provider id"),
            "copy-contract",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new()
            .with(ProviderOperation::Stat)
            .with(ProviderOperation::OpenReader)
            .with(ProviderOperation::OpenWriter),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Read)
            .with_guaranteed(FileSystemCapability::Write),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("provider properties should be valid")
}

/// Creates the opened handle metadata returned by the fixture.
fn info(path: &Path) -> OpenedFileInfo {
    OpenedFileInfo::new(
        FileSystemId::new("copy-contract").expect("valid provider id"),
        path.clone(),
    )
}

/// Builds a filesystem and shared observer for a selected failure/publication
/// mode.
fn filesystem(scenario: ReadScenario, mode: WriterMode) -> (FileSystem, Arc<Mutex<VisibleTarget>>) {
    let state = Arc::new(Mutex::new(VisibleTarget::default()));
    let filesystem = FileSystem::from_spi(CopyFixture {
        state: Arc::clone(&state),
        scenario,
        mode,
    })
    .expect("filesystem fixture should construct");
    (filesystem, state)
}

impl FileSystemSpi for CopyFixture {
    fn properties(&self) -> ProviderProperties {
        properties()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(None),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            info(request.path()),
            Box::new(ScriptedReader {
                scenario: self.scenario,
                position: 0,
            }),
        ))
    }

    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.state.lock().expect("target lock").created = true;
        Ok(OpenedWriter::new(
            info(request.path()),
            Box::new(SyncWriter {
                state: Arc::clone(&self.state),
                mode: self.mode,
            }),
        ))
    }
}

/// Emits a short byte prefix, then returns the configured read outcome.
struct ScriptedReader {
    scenario: ReadScenario,
    position: usize,
}

impl Input for ScriptedReader {
    type Item = u8;

    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> IoResult<usize> {
        if matches!(self.scenario, ReadScenario::FailImmediately) && self.position == 0 {
            return Err(IoError::other("injected read failure"));
        }
        if matches!(self.scenario, ReadScenario::FailAfterPrefix) && self.position >= 2 {
            return Err(IoError::other("injected read failure"));
        }
        if self.position >= 4 {
            return Ok(0);
        }
        let bytes = b"abcd";
        let accepted = count.min(2).min(bytes.len() - self.position);
        output[index..index + accepted].copy_from_slice(&bytes[self.position..self.position + accepted]);
        self.position += accepted;
        Ok(accepted)
    }
}

/// Writes directly to visible bytes or to a staging buffer according to mode.
struct SyncWriter {
    state: Arc<Mutex<VisibleTarget>>,
    mode: WriterMode,
}

impl Output for SyncWriter {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> IoResult<usize> {
        let accepted = count.min(2);
        let mut state = self.state.lock().expect("target lock");
        match self.mode {
            WriterMode::Direct => state.bytes.extend_from_slice(&input[index..index + accepted]),
            WriterMode::Staging => state.staging.extend_from_slice(&input[index..index + accepted]),
        }
        Ok(accepted)
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl FileWriterSpi for SyncWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        let mut state = self.state.lock().expect("target lock");
        state.commits += 1;
        if matches!(self.mode, WriterMode::Staging) {
            let staged = std::mem::take(&mut state.staging);
            state.bytes.extend_from_slice(&staged);
        }
        Ok(WriteOutcome::new(
            AchievedAtomicity::NonAtomic,
            PublicationMethod::StreamCopy,
        ))
    }

    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        let mut state = self.state.lock().expect("target lock");
        state.aborts += 1;
        match self.mode {
            WriterMode::Direct => Ok(WriteAbortOutcome::Indeterminate),
            WriterMode::Staging => {
                state.staging.clear();
                Ok(WriteAbortOutcome::NotPublished)
            }
        }
    }
}

/// Asserts visible publication, reported facts, and explicit recovery behavior.
fn assert_public_failure(
    scenario: ReadScenario,
    mode: WriterMode,
    max_bytes: Option<u64>,
    expected_error_kind: FsErrorKind,
    expected_visible_bytes: &[u8],
    expected_confirmed_bytes: u64,
) {
    let (filesystem, target) = filesystem(scenario, mode);
    let mut failure = filesystem
        .copy(
            &path("/source"),
            &path("/target"),
            CopyOptions::default().with_max_bytes(max_bytes),
        )
        .expect_err("scripted fallback should fail");
    let (created, visible_bytes, staging_bytes, commits) = {
        let visible = target.lock().expect("target lock");
        (
            visible.created,
            visible.bytes.clone(),
            visible.staging.clone(),
            visible.commits,
        )
    };
    assert!(created, "opening writer must be externally observable");
    assert_eq!(expected_visible_bytes, visible_bytes.as_slice(), "visible target bytes");
    if matches!(mode, WriterMode::Staging) {
        assert_eq!(b"ab", staging_bytes.as_slice(), "staging retains the accepted prefix");
    }
    assert_eq!(0, commits, "a failure must not commit implicitly");
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(expected_error_kind, failure.error().kind());
    assert_eq!(expected_confirmed_bytes, failure.partial_stats().bytes);
    assert_eq!(1, failure.partial_stats().failed);
    assert_eq!(Some(&path("/source")), failure.error().path());
    assert_eq!(Some(&path("/target")), failure.error().target());
    assert_eq!(Some("copy-contract"), failure.error().provider());
    assert!(failure.has_recovery());
    let failure_state = failure.state();
    let failure_stats = *failure.partial_stats();
    let abort_outcome = match failure.recovery_mut().expect("recovery writer is retained") {
        WriterRecovery::Opened(writer) => {
            assert_eq!(WriterState::Open, writer.state());
            writer.abort().expect("explicit abort should complete")
        }
        WriterRecovery::Rejected(_) => panic!("validated writer expected"),
    };
    assert_eq!(
        match mode {
            WriterMode::Direct => WriteAbortOutcome::Indeterminate,
            WriterMode::Staging => WriteAbortOutcome::NotPublished,
        },
        abort_outcome
    );
    assert_eq!(failure_state, failure.state());
    assert_eq!(failure_stats, *failure.partial_stats());
}

#[test]
fn test_copy_failure_reports_visible_target_and_recovery_facts() {
    assert_public_failure(
        ReadScenario::FailAfterPrefix,
        WriterMode::Direct,
        None,
        FsErrorKind::Io,
        b"ab",
        2,
    );
    assert_public_failure(
        ReadScenario::ExceedLimit,
        WriterMode::Direct,
        Some(3),
        FsErrorKind::ResourceLimitExceeded,
        b"ab",
        2,
    );
    assert_public_failure(
        ReadScenario::FailImmediately,
        WriterMode::Direct,
        None,
        FsErrorKind::Io,
        b"",
        0,
    );
    assert_public_failure(
        ReadScenario::FailAfterPrefix,
        WriterMode::Staging,
        None,
        FsErrorKind::Io,
        b"",
        2,
    );
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::copy::AsyncCopyOperationState;
    use qubit_fs::metadata::AchievedAtomicity;
    use qubit_fs::metadata::PublicationMethod;
    use qubit_fs::metadata::WriteOutcome;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncFileWriteSession;
    use qubit_fs::spi::OpenedAsyncReader;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::write::AsyncWriterRecovery;
    use qubit_fs::write::WriteFailure;
    use qubit_io::AsyncInput;
    use qubit_io::AsyncOutput;

    use super::CopyFailureState;
    use super::CopyFixture;
    use super::CopyOptions;
    use super::FileKind;
    use super::FileMetadata;
    use super::FsErrorKind;
    use super::FsResult;
    use super::IoError;
    use super::IoResult;
    use super::OpenReaderRequest;
    use super::OpenWriterRequest;
    use super::ProviderProperties;
    use super::ReadScenario;
    use super::SpiFuture;
    use super::StatRequest;
    use super::StatResponse;
    use super::VisibleTarget;
    use super::WriteAbortOutcome;
    use super::WriterMode;
    use super::WriterState;
    use super::info;
    use super::path;
    use super::properties;

    /// Builds an asynchronous filesystem and shared publication observer.
    fn async_filesystem(scenario: ReadScenario, mode: WriterMode) -> (AsyncFileSystem, Arc<Mutex<VisibleTarget>>) {
        let state = Arc::new(Mutex::new(VisibleTarget::default()));
        let filesystem = AsyncFileSystem::from_spi(CopyFixture {
            state: Arc::clone(&state),
            scenario,
            mode,
        })
        .expect("async filesystem fixture should construct");
        (filesystem, state)
    }

    impl AsyncFileSystemSpi for CopyFixture {
        fn properties(&self) -> ProviderProperties {
            properties()
        }

        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async move {
                Ok(StatResponse::new(
                    request.path().clone(),
                    FileMetadata::new(FileKind::File).with_len(None),
                ))
            })
        }

        fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
            let reader = AsyncScriptedReader {
                scenario: self.scenario,
                position: 0,
            };
            Box::pin(async move { Ok(OpenedAsyncReader::new(info(request.path()), Box::new(reader))) })
        }

        fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            self.state.lock().expect("target lock").created = true;
            let writer = AsyncFixtureWriter {
                state: Arc::clone(&self.state),
                mode: self.mode,
            };
            Box::pin(async move { Ok(OpenedAsyncWriter::new(info(request.path()), Box::new(writer))) })
        }
    }

    /// Emits a byte prefix asynchronously, then applies the selected read
    /// failure.
    struct AsyncScriptedReader {
        scenario: ReadScenario,
        position: usize,
    }

    impl AsyncInput for AsyncScriptedReader {
        type Item = u8;
        unsafe fn poll_read_unchecked(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            output: &mut [u8],
            index: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.as_mut().get_mut();
            if matches!(this.scenario, ReadScenario::FailImmediately) && this.position == 0
                || matches!(this.scenario, ReadScenario::FailAfterPrefix) && this.position >= 2
            {
                return Poll::Ready(Err(IoError::other("injected read failure")));
            }
            if this.position >= 4 {
                return Poll::Ready(Ok(0));
            }
            let accepted = count.min(2).min(4 - this.position);
            output[index..index + accepted].copy_from_slice(&b"abcd"[this.position..this.position + accepted]);
            this.position += accepted;
            Poll::Ready(Ok(accepted))
        }
    }

    /// Asynchronously writes into the direct destination or staging buffer.
    struct AsyncFixtureWriter {
        state: Arc<Mutex<VisibleTarget>>,
        mode: WriterMode,
    }

    impl AsyncOutput for AsyncFixtureWriter {
        type Item = u8;
        unsafe fn poll_write_unchecked(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            input: &[u8],
            index: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.as_mut().get_mut();
            let accepted = count.min(2);
            let mut state = this.state.lock().expect("target lock");
            match this.mode {
                WriterMode::Direct => state.bytes.extend_from_slice(&input[index..index + accepted]),
                WriterMode::Staging => state.staging.extend_from_slice(&input[index..index + accepted]),
            }
            Poll::Ready(Ok(accepted))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncFileWriteSession for AsyncFixtureWriter {
        fn commit_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
            let this = self.get_mut();
            let mut state = this.state.lock().expect("target lock");
            state.commits += 1;
            if matches!(this.mode, WriterMode::Staging) {
                let staged = std::mem::take(&mut state.staging);
                state.bytes.extend_from_slice(&staged);
            }
            Box::pin(async {
                Ok(WriteOutcome::new(
                    AchievedAtomicity::NonAtomic,
                    PublicationMethod::StreamCopy,
                ))
            })
        }
        fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<WriteAbortOutcome>> {
            let this = self.get_mut();
            let mut state = this.state.lock().expect("target lock");
            state.aborts += 1;
            let outcome = match this.mode {
                WriterMode::Direct => WriteAbortOutcome::Indeterminate,
                WriterMode::Staging => {
                    state.staging.clear();
                    WriteAbortOutcome::NotPublished
                }
            };
            Box::pin(async move { Ok(outcome) })
        }
        fn cancel_on_drop(self: Pin<&mut Self>) {}
    }

    /// Polls a ready test future until it completes.
    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
        }
    }

    /// Checks async publication state, failure facts, and explicit writer
    /// recovery.
    fn assert_async_failure(
        scenario: ReadScenario,
        mode: WriterMode,
        max_bytes: Option<u64>,
        expected_error_kind: FsErrorKind,
        expected_visible_bytes: &[u8],
        expected_confirmed_bytes: u64,
    ) {
        let (filesystem, target) = async_filesystem(scenario, mode);
        let mut operation = filesystem
            .begin_copy(
                path("/source"),
                path("/target"),
                CopyOptions::default().with_max_bytes(max_bytes),
            )
            .expect("copy preflight");
        let failure = ready(operation.execute()).expect_err("scripted fallback should fail");
        let (created, visible_bytes, staging_bytes, commits) = {
            let visible = target.lock().expect("target lock");
            (
                visible.created,
                visible.bytes.clone(),
                visible.staging.clone(),
                visible.commits,
            )
        };
        assert!(created);
        assert_eq!(expected_visible_bytes, visible_bytes.as_slice());
        if matches!(mode, WriterMode::Staging) {
            assert_eq!(b"ab", staging_bytes.as_slice());
        }
        assert_eq!(0, commits);
        assert_eq!(CopyFailureState::Indeterminate, failure.state());
        assert_eq!(expected_error_kind, failure.error().kind());
        assert_eq!(expected_confirmed_bytes, failure.partial_stats().bytes);
        assert_eq!(1, failure.partial_stats().failed);
        assert_eq!(Some(&path("/source")), failure.error().path());
        assert_eq!(Some(&path("/target")), failure.error().target());
        assert_eq!(Some("copy-contract"), failure.error().provider());
        let failure_state = failure.state();
        let failure_stats = *failure.partial_stats();
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate),
            operation.state()
        );
        assert!(operation.has_recovery());
        match operation.take_recovery().expect("recovery writer retained") {
            AsyncWriterRecovery::Opened(mut writer) => {
                assert_eq!(WriterState::Open, writer.state());
                let abort_outcome = ready(writer.abort_async()).expect("explicit async abort should complete");
                assert_eq!(
                    match mode {
                        WriterMode::Direct => WriteAbortOutcome::Indeterminate,
                        WriterMode::Staging => WriteAbortOutcome::NotPublished,
                    },
                    abort_outcome
                );
            }
            AsyncWriterRecovery::Rejected(_) => {
                panic!("validated writer expected")
            }
        }
        assert_eq!(failure_state, failure.state());
        assert_eq!(failure_stats, *failure.partial_stats());
        assert_eq!(
            AsyncCopyOperationState::Failed(CopyFailureState::Indeterminate),
            operation.state()
        );
    }

    #[test]
    fn test_async_copy_failure_reports_visible_target_and_recovery_facts() {
        assert_async_failure(
            ReadScenario::FailAfterPrefix,
            WriterMode::Direct,
            None,
            FsErrorKind::Io,
            b"ab",
            2,
        );
        assert_async_failure(
            ReadScenario::ExceedLimit,
            WriterMode::Direct,
            Some(3),
            FsErrorKind::ResourceLimitExceeded,
            b"ab",
            2,
        );
        assert_async_failure(
            ReadScenario::FailImmediately,
            WriterMode::Direct,
            None,
            FsErrorKind::Io,
            b"",
            0,
        );
        assert_async_failure(
            ReadScenario::FailAfterPrefix,
            WriterMode::Staging,
            None,
            FsErrorKind::Io,
            b"",
            2,
        );
    }
}
