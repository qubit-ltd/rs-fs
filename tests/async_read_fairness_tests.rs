// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cooperative scheduling contracts for asynchronous aggregate reads.

#![cfg(feature = "async")]

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

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
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
use qubit_fs::read::PrefixReadTermination;
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

#[derive(Default)]
struct Counts {
    calls: AtomicUsize,
    data_reads: AtomicUsize,
    wakes: AtomicUsize,
    drops: AtomicUsize,
}

struct CountWake(Arc<Counts>);

impl Wake for CountWake {
    fn wake(self: Arc<Self>) {
        self.0.wakes.fetch_add(1, Ordering::SeqCst);
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.wakes.fetch_add(1, Ordering::SeqCst);
    }
}

fn counter_waker(counts: &Arc<Counts>) -> Waker {
    Waker::from(Arc::new(CountWake(Arc::clone(counts))))
}

struct Script {
    bytes: Vec<u8>,
    pending_call: Option<usize>,
    error_call: Option<usize>,
    counts: Arc<Counts>,
}

impl Script {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            pending_call: None,
            error_call: None,
            counts: Arc::new(Counts::default()),
        }
    }

    fn pending_at(mut self, call: usize) -> Self {
        self.pending_call = Some(call);
        self
    }

    fn error_at(mut self, call: usize) -> Self {
        self.error_call = Some(call);
        self
    }
}

struct Provider(Script);

impl Provider {
    fn properties() -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("fairness").expect("provider id"),
                "fairness",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("properties")
    }

    fn info(path: &Path) -> OpenedFileInfo {
        OpenedFileInfo::new(
            FileSystemId::new("fairness").expect("provider id"),
            path.clone(),
        )
        .with_metadata(FileMetadata::new(FileKind::File))
    }
}

impl AsyncFileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        Self::properties()
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { panic!("unexpected stat") })
    }

    fn open_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        let reader = ScriptedReader {
            bytes: self.0.bytes.clone(),
            position: 0,
            pending_call: self.0.pending_call,
            error_call: self.0.error_call,
            pending_returned: false,
            counts: Arc::clone(&self.0.counts),
        };
        Box::pin(async move {
            Ok(OpenedAsyncReader::new(
                Self::info(request.path()),
                Box::new(reader),
            ))
        })
    }
}

struct ScriptedReader {
    bytes: Vec<u8>,
    position: usize,
    pending_call: Option<usize>,
    error_call: Option<usize>,
    pending_returned: bool,
    counts: Arc<Counts>,
}

impl Drop for ScriptedReader {
    fn drop(&mut self) {
        self.counts.drops.fetch_add(1, Ordering::SeqCst);
    }
}

impl AsyncInput for ScriptedReader {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<io::Result<usize>> {
        let this = self.as_mut().get_mut();
        let call = this.counts.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if this.pending_call == Some(call) && !this.pending_returned {
            this.pending_returned = true;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if this.error_call == Some(call) {
            return Poll::Ready(Err(io::Error::other("scripted read failure")));
        }
        if this.position == this.bytes.len() {
            return Poll::Ready(Ok(0));
        }
        let amount = count.min(1);
        output[index] = this.bytes[this.position];
        this.position += amount;
        this.counts.data_reads.fetch_add(amount, Ordering::SeqCst);
        Poll::Ready(Ok(amount))
    }
}

fn filesystem(script: Script) -> (AsyncFileSystem, Arc<Counts>) {
    let counts = Arc::clone(&script.counts);
    (
        AsyncFileSystem::from_spi(Provider(script)).expect("filesystem"),
        counts,
    )
}

fn path() -> Path {
    Path::parse("/payload").expect("path")
}

fn data(length: usize) -> Vec<u8> {
    (0..length).map(|index| (index % 251) as u8).collect()
}

#[test]
fn read_all_yields_after_each_64_successful_reads() {
    let input = data(130);
    let (fs, counts) = filesystem(Script::new(input.clone()));
    let resource_path = path();
    let operation = fs.read_all(&resource_path, ReadOptions::default(), 130);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(operation);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
    assert!(counts.wakes.load(Ordering::SeqCst) > 0);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 128);
    let result = match future.as_mut().poll(&mut cx) {
        Poll::Ready(result) => result.expect("successful aggregate read"),
        Poll::Pending => panic!("130 one-byte reads finish in three polls"),
    };
    assert_eq!(result, input);
}

#[test]
fn read_prefix_yields_after_each_64_successful_reads() {
    let input = data(130);
    let (fs, counts) = filesystem(Script::new(input.clone()));
    let resource_path = path();
    let operation = fs.read_prefix(&resource_path, ReadOptions::default(), 130);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(operation);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
    assert!(counts.wakes.load(Ordering::SeqCst) > 0);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 128);
    let result = match future.as_mut().poll(&mut cx) {
        Poll::Ready(result) => result.expect("successful prefix read"),
        Poll::Pending => panic!("130 one-byte reads finish in three polls"),
    };
    assert_eq!(result.bytes(), input);
    assert_eq!(result.termination(), PrefixReadTermination::LimitReached);
}

#[test]
fn prefix_limit_boundaries_preserve_completion_and_yield_semantics() {
    for limit in [0, 1, 63, 64, 65] {
        let input = data(80);
        let (fs, counts) = filesystem(Script::new(input.clone()));
        let resource_path = path();
        let operation = fs.read_prefix(&resource_path, ReadOptions::default(), limit);
        let waker = counter_waker(&counts);
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(operation);
        let first = future.as_mut().poll(&mut cx);
        let result = if limit == 65 {
            assert!(first.is_pending(), "65 reads yield before the next read");
            assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(result) => result,
                Poll::Pending => panic!("65-byte prefix completes in the second poll"),
            }
        } else {
            match first {
                Poll::Ready(result) => result,
                Poll::Pending => panic!("limit {limit} completes without yielding"),
            }
        }
        .expect("prefix read");
        assert_eq!(result.bytes(), &input[..limit]);
        assert_eq!(result.termination(), PrefixReadTermination::LimitReached);
        assert_eq!(counts.data_reads.load(Ordering::SeqCst), limit);
    }
}

#[test]
fn read_all_boundary_lengths_and_eof_probe_follow_yield_contract() {
    for length in [63, 64, 65] {
        let input = data(length);
        let (fs, counts) = filesystem(Script::new(input.clone()));
        let resource_path = path();
        let operation = fs.read_all(&resource_path, ReadOptions::default(), length);
        let waker = counter_waker(&counts);
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(operation);
        let first = future.as_mut().poll(&mut cx);
        let result = if length <= 63 {
            assert_eq!(counts.calls.load(Ordering::SeqCst), length + 1);
            match first {
                Poll::Ready(result) => result,
                Poll::Pending => panic!("length {length} completes without yielding"),
            }
        } else {
            assert!(first.is_pending());
            assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(result) => result,
                Poll::Pending => panic!("64-byte read and EOF probe complete in second poll"),
            }
        };
        assert_eq!(result.expect("read_all"), input);
    }
}

#[test]
fn short_stream_reaches_eof_without_extra_yield() {
    let input = data(7);
    let (fs, counts) = filesystem(Script::new(input.clone()));
    let resource_path = path();
    let operation = fs.read_all(&resource_path, ReadOptions::default(), 20);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(operation);
    let Poll::Ready(result) = future.as_mut().poll(&mut cx) else {
        panic!("short input completes in one poll")
    };
    assert_eq!(result.expect("short stream"), input);
    assert_eq!(counts.calls.load(Ordering::SeqCst), 8);
}

#[test]
fn read_all_rejects_bytes_beyond_budget() {
    let (fs, counts) = filesystem(Script::new(data(5)));
    let resource_path = path();
    let operation = fs.read_all(&resource_path, ReadOptions::default(), 2);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(operation);
    let Poll::Ready(result) = future.as_mut().poll(&mut cx) else {
        panic!("small overrun is reported immediately")
    };
    let error = result.expect_err("over-budget input");
    assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
    assert_eq!(error.path(), Some(&path()));
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 3);
}

#[test]
fn provider_pending_registers_waker_and_preserves_read_budget() {
    let input = data(130);
    let (fs, counts) = filesystem(Script::new(input.clone()).pending_at(1));
    let resource_path = path();
    let operation = fs.read_all(&resource_path, ReadOptions::default(), 130);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(operation);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.calls.load(Ordering::SeqCst), 1);
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 0);
    assert!(counts.wakes.load(Ordering::SeqCst) > 0);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 128);
    let Poll::Ready(result) = future.as_mut().poll(&mut cx) else {
        panic!("read completes")
    };
    assert_eq!(result.expect("read result"), input);
}

#[test]
fn io_errors_before_and_after_a_yield_keep_path_and_kind() {
    for error_call in [1, 65] {
        let (fs, counts) = filesystem(Script::new(data(130)).error_at(error_call));
        let resource_path = path();
        let operation = fs.read_all(&resource_path, ReadOptions::default(), 130);
        let waker = counter_waker(&counts);
        let mut cx = Context::from_waker(&waker);
        let mut future = std::pin::pin!(operation);
        if error_call == 65 {
            assert!(future.as_mut().poll(&mut cx).is_pending());
            assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
        }
        let Poll::Ready(result) = future.as_mut().poll(&mut cx) else {
            panic!("injected I/O error returns immediately")
        };
        let error = result.expect_err("scripted error");
        assert_eq!(error.kind(), FsErrorKind::Io);
        assert_eq!(error.path(), Some(&path()));
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn dropping_after_first_yield_releases_reader_without_more_reads() {
    let (fs, counts) = filesystem(Script::new(data(130)));
    let resource_path = path();
    let operation = fs.read_all(&resource_path, ReadOptions::default(), 130);
    let waker = counter_waker(&counts);
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(operation);
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
    drop(future);
    assert_eq!(counts.drops.load(Ordering::SeqCst), 1);
    let calls = counts.calls.load(Ordering::SeqCst);
    assert_eq!(counts.data_reads.load(Ordering::SeqCst), 64);
    assert_eq!(counts.calls.load(Ordering::SeqCst), calls);
}

#[test]
fn two_aggregate_futures_make_independent_progress_when_alternated() {
    let input = data(130);
    let (fs_a, counts_a) = filesystem(Script::new(input.clone()));
    let (fs_b, counts_b) = filesystem(Script::new(input.clone()));
    let resource_path_a = path();
    let resource_path_b = path();
    let operation_a = fs_a.read_all(&resource_path_a, ReadOptions::default(), 130);
    let operation_b = fs_b.read_prefix(&resource_path_b, ReadOptions::default(), 130);
    let waker_a = counter_waker(&counts_a);
    let waker_b = counter_waker(&counts_b);
    let mut cx_a = Context::from_waker(&waker_a);
    let mut cx_b = Context::from_waker(&waker_b);
    let mut future_a = std::pin::pin!(operation_a);
    let mut future_b = std::pin::pin!(operation_b);
    assert!(future_a.as_mut().poll(&mut cx_a).is_pending());
    assert!(future_b.as_mut().poll(&mut cx_b).is_pending());
    assert_eq!(counts_a.data_reads.load(Ordering::SeqCst), 64);
    assert_eq!(counts_b.data_reads.load(Ordering::SeqCst), 64);
    assert!(future_b.as_mut().poll(&mut cx_b).is_pending());
    assert!(future_a.as_mut().poll(&mut cx_a).is_pending());
    assert_eq!(counts_a.data_reads.load(Ordering::SeqCst), 128);
    assert_eq!(counts_b.data_reads.load(Ordering::SeqCst), 128);
    let Poll::Ready(result_a) = future_a.as_mut().poll(&mut cx_a) else {
        panic!("read_all complete")
    };
    let Poll::Ready(result_b) = future_b.as_mut().poll(&mut cx_b) else {
        panic!("read_prefix complete")
    };
    assert_eq!(result_a.expect("read_all"), input);
    assert_eq!(result_b.expect("read_prefix").bytes(), input);
}
