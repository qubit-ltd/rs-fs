// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use qubit_fs::{
    AchievedAtomicity, AsyncFileWriteSession, AsyncFileWriter, FileLocation, FileSystemId, FsError,
    FsErrorKind, FsFuture, FsOperation, FsPath, OpenedFileInfo, PublicationMethod, WriteFailure,
    WriteFailureState, WriteFuture, WriteOutcome, WriterState,
};
use qubit_io::AsyncOutput;

#[derive(Debug)]
struct ReadyWriteSession {
    bytes: Arc<Mutex<Vec<u8>>>,
    commit_failure_once: Option<WriteFailureState>,
    abort_error: Option<FsErrorKind>,
    buffered: bool,
    drop_cancellations: Arc<Mutex<usize>>,
}

impl AsyncOutput for ReadyWriteSession {
    type Item = u8;

    fn is_buffered(&self) -> bool {
        self.buffered
    }

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        self.get_mut()
            .bytes
            .lock()
            .expect("bytes lock should succeed")
            .extend_from_slice(&input[index..index + count]);
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for ReadyWriteSession {
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> WriteFuture<'a> {
        let this = self.get_mut();
        if let Some(state) = this.commit_failure_once.take() {
            let kind = match state {
                WriteFailureState::Indeterminate => FsErrorKind::Indeterminate,
                WriteFailureState::Retryable
                | WriteFailureState::NotPublished
                | WriteFailureState::Published => FsErrorKind::Io,
            };
            return Box::pin(async move {
                Err(WriteFailure::new(
                    FsError::new(kind, FsOperation::CommitWriter, "commit failed"),
                    state,
                ))
            });
        }
        Box::pin(async {
            Ok(WriteOutcome::new(
                AchievedAtomicity::Atomic,
                PublicationMethod::Direct,
            ))
        })
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        let abort_error = self.abort_error;
        Box::pin(async move {
            if let Some(kind) = abort_error {
                Err(FsError::new(kind, FsOperation::AbortWriter, "abort failed"))
            } else {
                Ok(())
            }
        })
    }

    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .drop_cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

fn assert_pending<F>(mut future: Pin<&mut F>)
where
    F: Future + ?Sized,
{
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
}

fn opened_info() -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("async-instance").expect("id should parse"),
        FsPath::parse_normalized("/out.bin").expect("path should parse"),
    ))
}

#[test]
fn async_commit_failure_retains_session_for_retry() {
    let mut writer = AsyncFileWriter::new(
        ReadyWriteSession {
            bytes: Arc::new(Mutex::new(Vec::new())),
            commit_failure_once: Some(WriteFailureState::Retryable),
            abort_error: None,
            buffered: false,
            drop_cancellations: Arc::new(Mutex::new(0)),
        },
        opened_info(),
    );

    assert!(ready(writer.commit_async()).is_err());
    assert_eq!(WriterState::Open, writer.state());
    assert!(ready(writer.commit_async()).is_ok());
    assert_eq!(WriterState::Committed, writer.state());
}

#[test]
fn async_commit_failure_preserves_definitive_publication_state() {
    for (failure_state, expected_writer_state) in [
        (WriteFailureState::NotPublished, WriterState::NotPublished),
        (WriteFailureState::Published, WriterState::Published),
    ] {
        let mut writer = AsyncFileWriter::new(
            ReadyWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: Some(failure_state),
                abort_error: None,
                buffered: false,
                drop_cancellations: Arc::new(Mutex::new(0)),
            },
            opened_info(),
        );

        assert_eq!(
            FsErrorKind::Io,
            ready(writer.commit_async()).unwrap_err().kind()
        );
        assert_eq!(expected_writer_state, writer.state());
        ready(writer.abort_async()).expect("definitive commit failure should remain abortable");
        assert_eq!(WriterState::Aborted, writer.state());
    }
}

#[test]
fn async_writer_drop_only_invokes_nonblocking_local_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let _writer = AsyncFileWriter::new(
            ReadyWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: None,
                buffered: false,
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );
    }

    assert_eq!(1, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_writer_drop_cancels_definitive_failed_sessions() {
    for failure_state in [
        WriteFailureState::NotPublished,
        WriteFailureState::Published,
    ] {
        let cancellations = Arc::new(Mutex::new(0));
        {
            let mut writer = AsyncFileWriter::new(
                ReadyWriteSession {
                    bytes: Arc::new(Mutex::new(Vec::new())),
                    commit_failure_once: Some(failure_state),
                    abort_error: None,
                    buffered: false,
                    drop_cancellations: cancellations.clone(),
                },
                opened_info(),
            );

            assert!(ready(writer.commit_async()).is_err());
        }
        assert_eq!(
            1,
            *cancellations.lock().expect("lock should succeed"),
            "dropping a definitively failed session should cancel local state",
        );
    }
}

#[test]
fn async_writer_forwards_io_and_rejects_it_after_commit() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let mut writer = AsyncFileWriter::new(
        ReadyWriteSession {
            bytes: bytes.clone(),
            commit_failure_once: None,
            abort_error: None,
            buffered: true,
            drop_cancellations: Arc::new(Mutex::new(0)),
        },
        opened_info(),
    );
    let mut context = Context::from_waker(Waker::noop());

    assert!(writer.is_buffered());
    assert_eq!("/out.bin", writer.info().location().path().as_str());
    assert!(format!("{writer:?}").contains("Open"));
    assert!(matches!(
        Pin::new(&mut writer).poll_write(&mut context, b"abc"),
        Poll::Ready(Ok(3)),
    ));
    assert!(matches!(
        Pin::new(&mut writer).poll_flush(&mut context),
        Poll::Ready(Ok(())),
    ));
    assert_eq!(
        b"abc",
        bytes.lock().expect("lock should succeed").as_slice()
    );

    ready(writer.commit_async()).expect("commit should succeed");
    assert_eq!(WriterState::Committed, writer.state());
    let Poll::Ready(Err(write_error)) = Pin::new(&mut writer).poll_write(&mut context, b"late")
    else {
        panic!("closed writer should reject I/O");
    };
    assert_eq!(std::io::ErrorKind::BrokenPipe, write_error.kind());
    assert_eq!(
        FsOperation::Write,
        write_error
            .get_ref()
            .and_then(|error| error.downcast_ref::<FsError>())
            .expect("stream error should retain FsError")
            .operation(),
    );
    assert!(matches!(
        Pin::new(&mut writer).poll_flush(&mut context),
        Poll::Ready(Err(error)) if error.kind() == std::io::ErrorKind::BrokenPipe,
    ));
    assert_eq!(
        FsErrorKind::InvalidState,
        ready(writer.commit_async()).unwrap_err().kind(),
    );
    assert_eq!(
        FsErrorKind::InvalidState,
        ready(writer.abort_async()).unwrap_err().kind(),
    );
}

#[test]
fn async_indeterminate_commit_can_be_aborted_without_drop_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let mut writer = AsyncFileWriter::new(
            ReadyWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: Some(WriteFailureState::Indeterminate),
                abort_error: None,
                buffered: false,
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );

        assert_eq!(
            FsErrorKind::Indeterminate,
            ready(writer.commit_async()).unwrap_err().kind(),
        );
        assert_eq!(WriterState::Indeterminate, writer.state());
        ready(writer.abort_async()).expect("abort should succeed");
        assert_eq!(WriterState::Aborted, writer.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_abort_failure_retains_open_state_for_drop_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let mut writer = AsyncFileWriter::new(
            ReadyWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: Some(FsErrorKind::Io),
                buffered: false,
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );

        assert!(ready(writer.abort_async()).is_err());
        assert_eq!(WriterState::Open, writer.state());
    }
    assert_eq!(1, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_indeterminate_abort_disables_drop_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let mut writer = AsyncFileWriter::new(
            ReadyWriteSession {
                bytes: Arc::new(Mutex::new(Vec::new())),
                commit_failure_once: None,
                abort_error: Some(FsErrorKind::Indeterminate),
                buffered: false,
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );

        assert_eq!(
            FsErrorKind::Indeterminate,
            ready(writer.abort_async()).unwrap_err().kind(),
        );
        assert_eq!(WriterState::Indeterminate, writer.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[derive(Debug)]
struct PendingLifecycleSession {
    drop_cancellations: Arc<Mutex<usize>>,
}

impl AsyncOutput for PendingLifecycleSession {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _input: &[u8],
        _index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for PendingLifecycleSession {
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> WriteFuture<'a> {
        Box::pin(std::future::pending())
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(std::future::pending())
    }

    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .drop_cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

#[test]
fn dropping_polled_pending_writer_lifecycle_futures_is_indeterminate() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let mut writer = AsyncFileWriter::new(
            PendingLifecycleSession {
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );
        let mut future = writer.commit_async();
        assert_pending(future.as_mut());
        drop(future);
        assert_eq!(WriterState::Indeterminate, writer.state());
    }
    {
        let mut writer = AsyncFileWriter::new(
            PendingLifecycleSession {
                drop_cancellations: cancellations.clone(),
            },
            opened_info(),
        );
        let mut future = writer.abort_async();
        assert_pending(future.as_mut());
        drop(future);
        assert_eq!(WriterState::Indeterminate, writer.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

struct DefaultCancellationSession;

impl AsyncOutput for DefaultCancellationSession {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _input: &[u8],
        _index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(count))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for DefaultCancellationSession {
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> WriteFuture<'a> {
        Box::pin(async {
            Ok(WriteOutcome::new(
                AchievedAtomicity::Atomic,
                PublicationMethod::Direct,
            ))
        })
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn async_session_default_drop_cancellation_is_nonblocking_noop() {
    let _writer = AsyncFileWriter::new(DefaultCancellationSession, opened_info());
}
