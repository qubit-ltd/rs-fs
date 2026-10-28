// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Verifies that a provider cannot over-report accepted writer bytes.

use std::io;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::metadata::WriteOutcome;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterState;
use qubit_io::Output;

const PROVIDER: &str = "overcount-test";

#[derive(Default)]
struct Probe {
    writes: AtomicUsize,
    flushes: AtomicUsize,
    commits: AtomicUsize,
    aborts: AtomicUsize,
    #[cfg(feature = "async")]
    async_commit_failure: AtomicUsize,
    #[cfg(feature = "async")]
    async_abort_outcome: AtomicUsize,
    #[cfg(feature = "async")]
    async_expected_bytes: AtomicUsize,
    accepted: std::sync::Mutex<Vec<u8>>,
}

struct SyncFs(Arc<Probe>);
struct SyncWriter(Arc<Probe>);

fn properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new(PROVIDER).expect("provider id"),
            "overcount test provider",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new().with(ProviderOperation::OpenWriter),
        FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Write),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("provider properties")
}

impl FileSystemSpi for SyncFs {
    fn properties(&self) -> ProviderProperties {
        properties()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        let info = OpenedFileInfo::new(FileSystemId::new(PROVIDER).unwrap(), request.path().clone());
        Ok(OpenedWriter::new(info, Box::new(SyncWriter(Arc::clone(&self.0)))))
    }
}

impl Output for SyncWriter {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> IoResult<usize> {
        if count == 0 {
            self.0.writes.fetch_add(1, Ordering::SeqCst);
            return Ok(1);
        }
        let call = self.0.writes.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            let accepted = count.min(2);
            self.0
                .accepted
                .lock()
                .unwrap()
                .extend_from_slice(&input[index..index + accepted]);
            Ok(accepted)
        } else if count == 0 {
            Ok(1)
        } else {
            Ok(count + 1)
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        self.0.flushes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl FileWriterSpi for SyncWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        self.0.commits.fetch_add(1, Ordering::SeqCst);
        Ok(WriteOutcome::new(
            qubit_fs::metadata::AchievedAtomicity::Atomic,
            qubit_fs::metadata::PublicationMethod::AtomicRename,
        ))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        self.0.aborts.fetch_add(1, Ordering::SeqCst);
        Ok(WriteAbortOutcome::Indeterminate)
    }
}

fn assert_contract_error(error: &io::Error, path: &Path) {
    let typed = error
        .get_ref()
        .and_then(|source| source.downcast_ref::<FsError>())
        .expect("IO error should retain typed FsError");
    assert_eq!(FsErrorKind::ProviderContractViolation, typed.kind());
    assert_eq!(FsOperation::Write, typed.operation());
    assert_eq!(Some(FsEffectState::Indeterminate), typed.effect_state());
    assert_eq!(Some(path), typed.path());
    assert_eq!(Some("overcount test provider"), typed.provider());
}

#[test]
fn test_sync_overreported_write_poisoning_blocks_commit() {
    let probe = Arc::new(Probe::default());
    let fs = FileSystem::from_spi(SyncFs(Arc::clone(&probe))).unwrap();
    let path = Path::parse("/count-sync").unwrap();
    let mut writer = fs.open_writer(&path, WriteOptions::default()).unwrap();
    let payload = b"abcdef";
    assert_eq!(2, unsafe { writer.write_unchecked(payload, 0, payload.len()) }.unwrap());
    let error = unsafe { writer.write_unchecked(payload, 2, payload.len() - 2) }.unwrap_err();
    assert_contract_error(&error, &path);
    assert_eq!(b"ab", probe.accepted.lock().unwrap().as_slice());
    assert_eq!(WriterState::Indeterminate, writer.state());
    assert!(unsafe { writer.write_unchecked(payload, 0, 1) }.is_err());
    assert!(writer.flush().is_err());
    assert!(writer.commit().is_err());
    assert_eq!(2, probe.writes.load(Ordering::SeqCst));
    assert_eq!(0, probe.flushes.load(Ordering::SeqCst));
    assert_eq!(0, probe.commits.load(Ordering::SeqCst));
    assert_eq!(WriteAbortOutcome::Indeterminate, writer.abort().unwrap());
    assert_eq!(WriterState::Indeterminate, writer.state());
    assert_eq!(1, probe.aborts.load(Ordering::SeqCst));
}

#[test]
fn test_sync_zero_length_overreported_write_is_rejected() {
    let probe = Arc::new(Probe::default());
    let fs = FileSystem::from_spi(SyncFs(Arc::clone(&probe))).unwrap();
    let path = Path::parse("/count-zero").unwrap();
    let mut writer = fs.open_writer(&path, WriteOptions::default()).unwrap();
    let error = unsafe { writer.write_unchecked(b"", 0, 0) }.unwrap_err();
    assert_contract_error(&error, &path);
    assert_eq!(WriterState::Indeterminate, writer.state());
    assert_eq!(0, probe.commits.load(Ordering::SeqCst));
}

#[test]
fn test_sync_write_all_overcount_retains_recovery_and_prefix_count() {
    let probe = Arc::new(Probe::default());
    let fs = FileSystem::from_spi(SyncFs(Arc::clone(&probe))).unwrap();
    let path = Path::parse("/count-write-all").unwrap();
    let failure = fs.write_all(&path, b"abcdef", WriteOptions::default()).unwrap_err();
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(FsOperation::Write, failure.error().operation());
    assert_eq!(Some(FsEffectState::Indeterminate), failure.error().effect_state());
    assert_eq!(Some(&path), failure.error().path());
    assert_eq!(Some("overcount test provider"), failure.error().provider());
    assert_eq!(2, failure.written_bytes());
    let recovery = failure.recovery().expect("writer recovery retained");
    let writer = match recovery {
        qubit_fs::write::WriterRecovery::Opened(writer) => writer,
        qubit_fs::write::WriterRecovery::Rejected(_) => panic!("writer was validated"),
    };
    assert_eq!(WriterState::Indeterminate, writer.state());
    assert_eq!(b"ab", probe.accepted.lock().unwrap().as_slice());
    assert_eq!(0, probe.commits.load(Ordering::SeqCst));
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncFileWriteSession;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::write::AsyncWriterRecovery;
    use qubit_io::AsyncOutput;

    use super::Arc;
    use super::FileKind;
    use super::FileMetadata;
    use super::FileSystemId;
    use super::FsEffectState;
    use super::FsError;
    use super::FsErrorKind;
    use super::FsOperation;
    use super::FsResult;
    use super::IoResult;
    use super::OpenWriterRequest;
    use super::OpenedFileInfo;
    use super::Ordering;
    use super::PROVIDER;
    use super::Path;
    use super::Probe;
    use super::ProviderProperties;
    use super::StatRequest;
    use super::StatResponse;
    use super::WriteAbortOutcome;
    use super::WriteOptions;
    use super::WriteOutcome;
    use super::WriterState;
    use super::io;
    use super::properties;

    struct AsyncFs(Arc<Probe>);
    struct AsyncWriter(Arc<Probe>);

    impl AsyncFileSystemSpi for AsyncFs {
        fn properties(&self) -> ProviderProperties {
            properties()
        }
        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            let path = request.path().clone();
            Box::pin(async move { Ok(StatResponse::new(path, FileMetadata::new(FileKind::File))) })
        }
        fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            let path = request.path().clone();
            let probe = Arc::clone(&self.0);
            Box::pin(async move {
                let info = OpenedFileInfo::new(FileSystemId::new(PROVIDER).unwrap(), path);
                Ok(OpenedAsyncWriter::new(info, Box::new(AsyncWriter(probe))))
            })
        }
    }

    impl AsyncOutput for AsyncWriter {
        type Item = u8;
        unsafe fn poll_write_unchecked(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            input: &[u8],
            index: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.get_mut();
            if count == 0 {
                this.0.writes.fetch_add(1, Ordering::SeqCst);
                return Poll::Ready(Ok(1));
            }
            let call = this.0.writes.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                let accepted = count.min(2);
                this.0
                    .accepted
                    .lock()
                    .unwrap()
                    .extend_from_slice(&input[index..index + accepted]);
                Poll::Ready(Ok(accepted))
            } else if count == 0 {
                Poll::Ready(Ok(1))
            } else {
                Poll::Ready(Ok(count + 1))
            }
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            self.get_mut().0.flushes.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        }
    }
    impl AsyncFileWriteSession for AsyncWriter {
        fn commit_async<'a>(
            self: Pin<&'a mut Self>,
        ) -> SpiFuture<'a, Result<WriteOutcome, qubit_fs::write::WriteFailure>> {
            let probe = Arc::clone(&self.get_mut().0);
            let attempt = probe.commits.fetch_add(1, Ordering::SeqCst);
            let failure = probe.async_commit_failure.load(Ordering::SeqCst);
            Box::pin(async move {
                if failure != 0 && attempt == 0 {
                    let state = match failure {
                        1 => qubit_fs::write::WriteFailureState::RetryableNotPublished,
                        2 => qubit_fs::write::WriteFailureState::NotPublished,
                        3 => qubit_fs::write::WriteFailureState::Published,
                        _ => qubit_fs::write::WriteFailureState::Indeterminate,
                    };
                    return Err(qubit_fs::write::WriteFailure::new(
                        FsError::new(FsErrorKind::Io, FsOperation::CommitWriter, "commit probe"),
                        state,
                    ));
                }
                Ok(WriteOutcome::new(
                    qubit_fs::metadata::AchievedAtomicity::Atomic,
                    qubit_fs::metadata::PublicationMethod::AtomicRename,
                )
                .with_bytes_written(probe.async_expected_bytes.load(Ordering::SeqCst) as u64))
            })
        }
        fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<WriteAbortOutcome>> {
            let probe = Arc::clone(&self.get_mut().0);
            probe.aborts.fetch_add(1, Ordering::SeqCst);
            let outcome = probe.async_abort_outcome.load(Ordering::SeqCst);
            Box::pin(async move {
                Ok(match outcome {
                    1 => WriteAbortOutcome::NotPublished,
                    2 => WriteAbortOutcome::Published,
                    _ => WriteAbortOutcome::Indeterminate,
                })
            })
        }
    }

    fn ready<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        let mut cx = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("fixture future unexpectedly pending"),
        }
    }
    fn checked(error: &io::Error, path: &Path) {
        let typed = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<FsError>())
            .unwrap();
        assert_eq!(FsErrorKind::ProviderContractViolation, typed.kind());
        assert_eq!(FsOperation::Write, typed.operation());
        assert_eq!(Some(FsEffectState::Indeterminate), typed.effect_state());
        assert_eq!(Some(path), typed.path());
        assert_eq!(Some("overcount test provider"), typed.provider());
    }

    #[test]
    fn test_async_overreported_write_poisoning_blocks_commit() {
        let probe = Arc::new(Probe::default());
        let fs = AsyncFileSystem::from_spi(AsyncFs(Arc::clone(&probe))).unwrap();
        let path = Path::parse("/count-async").unwrap();
        ready(async {
            let mut writer = fs.open_writer(&path, WriteOptions::default()).await.unwrap();
            let bytes = b"abcdef";
            let mut cx = Context::from_waker(Waker::noop());
            let first = unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, bytes, 0, 6) };
            assert!(matches!(first, Poll::Ready(Ok(2))));
            let result = unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, bytes, 2, 4) };
            let error = match result {
                Poll::Ready(Err(error)) => error,
                _ => panic!("overcount must fail"),
            };
            checked(&error, &path);
            assert_eq!(b"ab", probe.accepted.lock().unwrap().as_slice());
            assert_eq!(WriterState::Indeterminate, writer.state());
            assert!(matches!(
                unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, bytes, 0, 1) },
                Poll::Ready(Err(_))
            ));
            assert!(matches!(Pin::new(&mut writer).poll_flush(&mut cx), Poll::Ready(Err(_))));
            assert!(ready(writer.commit_async()).is_err());
            assert_eq!(2, probe.writes.load(Ordering::SeqCst));
            assert_eq!(0, probe.flushes.load(Ordering::SeqCst));
            assert_eq!(0, probe.commits.load(Ordering::SeqCst));
            assert_eq!(WriteAbortOutcome::Indeterminate, ready(writer.abort_async()).unwrap());
            assert_eq!(WriterState::Indeterminate, writer.state());
            assert_eq!(1, probe.aborts.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn test_async_zero_length_write_does_not_call_provider() {
        let probe = Arc::new(Probe::default());
        let fs = AsyncFileSystem::from_spi(AsyncFs(Arc::clone(&probe))).unwrap();
        let path = Path::parse("/count-async-zero").unwrap();
        ready(async {
            let mut writer = fs.open_writer(&path, WriteOptions::default()).await.unwrap();
            let mut cx = Context::from_waker(Waker::noop());
            let state = writer.state();
            let result = unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, b"", 0, 0) };
            assert!(matches!(result, Poll::Ready(Ok(0))));
            assert_eq!(state, writer.state());
            assert_eq!(0, probe.writes.load(Ordering::SeqCst));
            assert_eq!(0, probe.commits.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn test_async_zero_length_write_preserves_lifecycle_states() {
        for (commit_failure, abort_outcome, expected) in [
            (0, 0, WriterState::Open),
            (1, 0, WriterState::Open),
            (0, 0, WriterState::Committed),
            (2, 0, WriterState::NotPublished),
            (3, 0, WriterState::Published),
            (4, 0, WriterState::Indeterminate),
            (0, 1, WriterState::Aborted),
        ] {
            let probe = Arc::new(Probe::default());
            probe.async_commit_failure.store(commit_failure, Ordering::SeqCst);
            probe.async_abort_outcome.store(abort_outcome, Ordering::SeqCst);
            let fs = AsyncFileSystem::from_spi(AsyncFs(Arc::clone(&probe))).unwrap();
            let path = Path::parse("/count-async-zero-state").unwrap();
            ready(async {
                let mut writer = fs.open_writer(&path, WriteOptions::default()).await.unwrap();
                if commit_failure != 0 {
                    assert!(ready(writer.commit_async()).is_err());
                } else if expected == WriterState::Committed {
                    ready(writer.commit_async()).unwrap();
                } else if expected == WriterState::Aborted {
                    assert_eq!(WriteAbortOutcome::NotPublished, ready(writer.abort_async()).unwrap());
                }

                let mut cx = Context::from_waker(Waker::noop());
                if expected == WriterState::Open {
                    probe.async_expected_bytes.store(1, Ordering::SeqCst);
                    assert!(matches!(
                        unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, b"x", 0, 1) },
                        Poll::Ready(Ok(1))
                    ));
                }

                let state = writer.state();
                let writes = probe.writes.load(Ordering::SeqCst);
                let flushes = probe.flushes.load(Ordering::SeqCst);
                let commits = probe.commits.load(Ordering::SeqCst);
                for (input, index) in [(&b""[..], 0), (&b"abc"[..], 3)] {
                    let result = unsafe { Pin::new(&mut writer).poll_write_unchecked(&mut cx, input, index, 0) };
                    assert!(matches!(result, Poll::Ready(Ok(0))));
                    assert_eq!(state, writer.state());
                    assert_eq!(writes, probe.writes.load(Ordering::SeqCst));
                    assert_eq!(flushes, probe.flushes.load(Ordering::SeqCst));
                    assert_eq!(commits, probe.commits.load(Ordering::SeqCst));
                }
                assert_eq!(expected, writer.state());
                if expected == WriterState::Open {
                    ready(writer.commit_async()).unwrap();
                }
            });
        }
    }

    #[test]
    fn test_async_write_all_overcount_retains_owned_recovery_and_prefix_count() {
        let probe = Arc::new(Probe::default());
        let fs = AsyncFileSystem::from_spi(AsyncFs(Arc::clone(&probe))).unwrap();
        let path = Path::parse("/count-async-all").unwrap();
        let mut operation = fs
            .begin_write_all(path.clone(), b"abcdef".to_vec(), WriteOptions::default())
            .unwrap();
        let error = ready(operation.execute()).unwrap_err();
        assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
        assert_eq!(FsOperation::Write, error.error().operation());
        assert_eq!(Some(FsEffectState::Indeterminate), error.error().effect_state());
        assert_eq!(Some(&path), error.error().path());
        assert_eq!(Some("overcount test provider"), error.error().provider());
        assert_eq!(2, operation.written_bytes());
        assert!(operation.has_recovery());
        match operation.recovery().expect("owned writer retained") {
            AsyncWriterRecovery::Opened(writer) => {
                assert_eq!(WriterState::Indeterminate, writer.state())
            }
            AsyncWriterRecovery::Rejected(_) => panic!("writer was validated"),
        }
        assert_eq!(b"ab", probe.accepted.lock().unwrap().as_slice());
        assert_eq!(0, probe.commits.load(Ordering::SeqCst));
    }
}
