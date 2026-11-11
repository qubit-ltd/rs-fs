// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#![cfg(feature = "async")]

use std::error::Error;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::thread;

use qubit_fs::AsyncFileSystem;
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
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_io::AsyncInput;

struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => thread::park(),
        }
    }
}

#[derive(Debug)]
struct Marker;
impl std::fmt::Display for Marker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("marker")
    }
}
impl Error for Marker {}

#[derive(Clone)]
enum Step {
    Error(io::ErrorKind),
    TypedInterrupted,
    Permission,
    PendingThenData,
}

struct ScriptedInput {
    step: Step,
    polls: Arc<AtomicUsize>,
    sent: bool,
    data_sent: bool,
}
impl AsyncInput for ScriptedInput {
    type Item = u8;
    fn is_buffered(&self) -> bool {
        false
    }
    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        this.polls.fetch_add(1, Ordering::SeqCst);
        match this.step {
            Step::Error(kind) => Poll::Ready(Err(io::Error::new(kind, "injected"))),
            Step::TypedInterrupted => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::Interrupted,
                FsError::with_source(FsErrorKind::PermissionDenied, FsOperation::Read, "typed", Marker)
                    .with_provider("script-provider")
                    .with_effect_state(FsEffectState::Applied),
            ))),
            Step::Permission => Poll::Ready(Err(io::Error::other(
                FsError::with_source(FsErrorKind::PermissionDenied, FsOperation::Read, "typed", Marker)
                    .with_provider("script-provider")
                    .with_effect_state(FsEffectState::Applied),
            ))),
            Step::PendingThenData if !this.sent => {
                this.sent = true;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Step::PendingThenData if !this.data_sent => {
                this.data_sent = true;
                let n = count.min(3);
                output[index..index + n].copy_from_slice(b"abc"[..n].as_ref());
                Poll::Ready(Ok(n))
            }
            Step::PendingThenData => Poll::Ready(Ok(0)),
        }
    }
}

struct ScriptSpi {
    step: Step,
    polls: Arc<AtomicUsize>,
}
impl ScriptSpi {
    fn props() -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-contract").unwrap(),
                "async-contract",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
}
impl AsyncFileSystemSpi for ScriptSpi {
    fn properties(&self) -> ProviderProperties {
        Self::props()
    }
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        let p = request.path().clone();
        Box::pin(async move { Ok(StatResponse::new(p, FileMetadata::new(FileKind::File))) })
    }
    fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        let info = OpenedFileInfo::new(FileSystemId::new("async-contract").unwrap(), request.path().clone())
            .with_metadata(FileMetadata::new(FileKind::File));
        let input = ScriptedInput {
            step: self.step.clone(),
            polls: Arc::clone(&self.polls),
            sent: false,
            data_sent: false,
        };
        Box::pin(async move { Ok(OpenedAsyncReader::new(info, Box::new(input))) })
    }
}

fn fs(step: Step) -> (AsyncFileSystem, Arc<AtomicUsize>) {
    let polls = Arc::new(AtomicUsize::new(0));
    (
        AsyncFileSystem::from_spi(ScriptSpi {
            step,
            polls: Arc::clone(&polls),
        })
        .expect("script provider should be valid"),
        polls,
    )
}
fn path() -> Path {
    Path::parse("/file").unwrap()
}
fn chain_has<T: Error + 'static>(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(source) = current {
        if source.is::<T>() {
            return true;
        }
        current = source.source();
    }
    false
}

#[test]
fn test_forbidden_errors_are_io_without_retry() {
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted] {
        for prefix in [false, true] {
            let (filesystem, polls) = fs(Step::Error(kind));
            let result = if prefix {
                run(filesystem.read_prefix(&path(), ReadOptions::default(), 3)).map(|_| vec![])
            } else {
                run(filesystem.read_all(&path(), ReadOptions::default(), 10))
            };
            let error = result.unwrap_err();
            assert_eq!(FsErrorKind::Io, error.kind());
            assert_eq!(1, polls.load(Ordering::SeqCst));
        }
    }
}

#[test]
fn test_normalized_typed_interrupted_is_io_and_preserves_cause_chain() {
    for prefix in [false, true] {
        let (filesystem, polls) = fs(Step::TypedInterrupted);
        let error = if prefix {
            run(filesystem.read_prefix(&path(), ReadOptions::default(), 3)).unwrap_err()
        } else {
            run(filesystem.read_all(&path(), ReadOptions::default(), 10)).unwrap_err()
        };
        assert_eq!(FsErrorKind::Io, error.kind());
        assert_eq!(1, polls.load(Ordering::SeqCst));
        assert!(chain_has::<Marker>(&error));
        let mut current: Option<&(dyn Error + 'static)> = Some(&error);
        let mut source_chain_has_marker = false;
        while let Some(source) = current {
            source_chain_has_marker |= source.is::<Marker>();
            current = source.source();
        }
        assert!(
            source_chain_has_marker,
            "the original source cause should remain observable"
        );
    }
}

#[test]
fn test_direct_typed_permission_error_is_preserved() {
    for prefix in [false, true] {
        let (filesystem, polls) = fs(Step::Permission);
        let error = if prefix {
            run(filesystem.read_prefix(&path(), ReadOptions::default(), 3)).unwrap_err()
        } else {
            run(filesystem.read_all(&path(), ReadOptions::default(), 10)).unwrap_err()
        };
        assert_eq!(FsErrorKind::PermissionDenied, error.kind());
        assert_eq!(Some("script-provider"), error.provider());
        assert_eq!(Some(FsEffectState::Applied), error.effect_state());
        assert!(chain_has::<Marker>(&error));
        assert_eq!(1, polls.load(Ordering::SeqCst));
    }
}

#[test]
fn test_pending_then_data_and_eof_succeeds() {
    let (filesystem, polls) = fs(Step::PendingThenData);
    let outcome = run(filesystem.read_all(&path(), ReadOptions::default(), 10)).unwrap();
    assert_eq!(b"abc", outcome.as_slice());
    assert_eq!(3, polls.load(Ordering::SeqCst));
}

#[test]
fn test_safe_reader_read_async_exposes_invalid_data_for_forbidden_error() {
    for kind in [io::ErrorKind::WouldBlock, io::ErrorKind::Interrupted] {
        let (filesystem, polls) = fs(Step::Error(kind));
        let mut reader = run(filesystem.open_reader(&path(), ReadOptions::default())).unwrap();
        let mut bytes = [0; 4];
        let error = run(reader.read_async(&mut bytes)).unwrap_err();
        assert_eq!(io::ErrorKind::InvalidData, error.kind());
        assert_eq!(1, polls.load(Ordering::SeqCst));
    }
}
