// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::AsyncRandomAccessReadSession;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedAsyncRandomAccessReader;
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::OpenedRandomAccessReader;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RandomAccessReadSession;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs_documentation_examples::random_access_sampling::sample;
use qubit_fs_documentation_examples::random_access_sampling_async::sample_async;

const ID: &str = "sampling-fixture";

#[derive(Default, Debug)]
struct Record {
    opens: usize,
    path_opens: usize,
    requests: Vec<usize>,
    reads: usize,
}

#[derive(Clone)]
struct Config {
    bytes: Arc<[u8]>,
    reported_len: u64,
    limit: FileSystemLimit,
    short: bool,
    error_at: Option<usize>,
    pending_first: bool,
    record: Arc<Mutex<Record>>,
}

impl Config {
    fn new(limit: FileSystemLimit) -> Self {
        Self {
            bytes: Arc::from(&b"abcdefgh"[..]),
            reported_len: 8,
            limit,
            short: false,
            error_at: None,
            pending_first: false,
            record: Arc::new(Mutex::new(Record::default())),
        }
    }
    fn filesystem(&self) -> FileSystem {
        FileSystem::from_spi(SyncProvider(self.clone())).unwrap()
    }
    fn async_filesystem(&self) -> AsyncFileSystem {
        AsyncFileSystem::from_spi(AsyncProvider(self.clone())).unwrap()
    }
    fn record(&self) -> Record {
        let record = self.record.lock().unwrap();
        Record {
            opens: record.opens,
            path_opens: record.path_opens,
            requests: record.requests.clone(),
            reads: record.reads,
        }
    }
}

fn props_with_limit(limit: FileSystemLimit) -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new(ID).unwrap(),
            "sampling fixture",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new()
            .with(ProviderOperation::OpenReader)
            .with(ProviderOperation::OpenRandomAccessReader),
        FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
        FileSystemLimits::unknown().with_max_read_range_bytes(limit),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .unwrap()
}

fn info(path: Path, len: u64) -> OpenedFileInfo {
    OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
        .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(len)))
}

struct SyncProvider(Config);
struct SyncSession(Config);

impl FileSystemSpi for SyncProvider {
    fn properties(&self) -> ProviderProperties {
        props_with_limit(self.0.limit)
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
    fn open_random_access_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedRandomAccessReader> {
        self.0.record.lock().unwrap().opens += 1;
        Ok(OpenedRandomAccessReader::new(
            info(request.path().clone(), self.0.reported_len),
            Box::new(SyncSession(self.0.clone())),
        ))
    }
    fn open_reader(&self, _request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.0.record.lock().unwrap().path_opens += 1;
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::OpenReader,
            "unexpected path-based open",
        ))
    }
}

impl RandomAccessReadSession for SyncSession {
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let call = {
            let mut record = self.0.record.lock().unwrap();
            let call = record.reads;
            record.reads += 1;
            record.requests.push(output.len());
            call
        };
        if self.0.error_at == Some(call) {
            if let Some(first) = output.first_mut() {
                *first = b'!';
            }
            return Err(FsError::new(FsErrorKind::Io, FsOperation::Read, "fixture read failure"));
        }
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        let Some(available) = self.0.bytes.get(start..) else {
            return Ok(0);
        };
        let mut count = output.len().min(available.len());
        if self.0.short {
            count = count.min(1);
        }
        output[..count].copy_from_slice(&available[..count]);
        Ok(count)
    }
}

struct AsyncProvider(Config);
struct AsyncSession(Config);

impl AsyncFileSystemSpi for AsyncProvider {
    fn properties(&self) -> ProviderProperties {
        props_with_limit(self.0.limit)
    }
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            Ok(StatResponse::new(
                request.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        })
    }
    fn open_random_access_reader<'a>(
        &'a self,
        request: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncRandomAccessReader>> {
        self.0.record.lock().unwrap().opens += 1;
        let config = self.0.clone();
        let path = request.path().clone();
        Box::pin(async move {
            Ok(OpenedAsyncRandomAccessReader::new(
                info(path, config.reported_len),
                Box::new(AsyncSession(config)),
            ))
        })
    }
    fn open_reader<'a>(&'a self, _request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        self.0.record.lock().unwrap().path_opens += 1;
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                FsOperation::OpenReader,
                "unexpected path-based open",
            ))
        })
    }
}

impl AsyncRandomAccessReadSession for AsyncSession {
    fn read_at_async<'a>(self: Pin<&'a mut Self>, offset: u64, output: &'a mut [u8]) -> SpiFuture<'a, FsResult<usize>> {
        Box::pin(async move {
            let this = self.get_mut();
            let (call, should_pending) = {
                let mut record = this.0.record.lock().unwrap();
                let call = record.reads;
                record.reads += 1;
                record.requests.push(output.len());
                (call, this.0.pending_first && call == 0)
            };
            if should_pending {
                if let Some(first) = output.first_mut() {
                    *first = b'!';
                }
                std::future::poll_fn(|_| Poll::<()>::Pending).await;
            }
            if this.0.error_at == Some(call) {
                if let Some(first) = output.first_mut() {
                    *first = b'!';
                }
                return Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::Read,
                    "fixture async read failure",
                ));
            }
            let start = usize::try_from(offset).unwrap_or(usize::MAX);
            let Some(available) = this.0.bytes.get(start..) else {
                return Ok(0);
            };
            let mut count = output.len().min(available.len());
            if this.0.short {
                count = count.min(1);
            }
            output[..count].copy_from_slice(&available[..count]);
            Ok(count)
        })
    }
}

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected a ready fixture operation"),
    }
}

fn path() -> Path {
    Path::parse("/sample").unwrap()
}

#[test]
fn test_sync_sampling_splits_at_limit_and_keeps_one_open_session() {
    let config = Config::new(FileSystemLimit::Maximum(3));
    let mut output = [0; 8];
    assert_eq!(8, sample(&config.filesystem(), &path(), 0, &mut output).unwrap());
    assert_eq!(*b"abcdefgh", output);
    let record = config.record();
    assert_eq!(1, record.opens);
    assert_eq!(0, record.path_opens);
    assert_eq!(vec![3, 3, 2], record.requests);
}

#[test]
fn test_sync_sampling_handles_short_reads_eof_empty_and_zero_limit() {
    let mut short = Config::new(FileSystemLimit::Maximum(3));
    short.short = true;
    let mut output = [0; 4];
    assert_eq!(4, sample(&short.filesystem(), &path(), 1, &mut output).unwrap());
    assert_eq!(*b"bcde", output);
    assert_eq!(vec![3, 3, 2, 1], short.record().requests);

    let mut eof = Config::new(FileSystemLimit::Maximum(3));
    eof.bytes = Arc::from(&b"abc"[..]);
    let mut output = [0; 8];
    assert_eq!(3, sample(&eof.filesystem(), &path(), 0, &mut output).unwrap());
    assert_eq!(*b"abc\0\0\0\0\0", output);
    assert_eq!(vec![3, 3], eof.record().requests);

    let zero = Config::new(FileSystemLimit::Maximum(0));
    assert_eq!(0, sample(&zero.filesystem(), &path(), 0, &mut []).unwrap());
    let error = sample(&zero.filesystem(), &path(), 0, &mut [0]).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(0, sample(&zero.filesystem(), &path(), 8, &mut [0]).unwrap());
    assert_eq!(0, zero.record().reads);
    assert_eq!(3, zero.record().opens);
}

#[test]
fn test_sync_sampling_accepts_nonfinite_limits_and_stops_on_errors_or_overflow() {
    for limit in [
        FileSystemLimit::Unknown,
        FileSystemLimit::Unbounded,
        FileSystemLimit::NotApplicable,
    ] {
        let config = Config::new(limit);
        let mut output = [0; 8];
        assert_eq!(8, sample(&config.filesystem(), &path(), 0, &mut output).unwrap());
        assert_eq!(vec![8], config.record().requests);
    }

    let mut failing = Config::new(FileSystemLimit::Maximum(2));
    failing.error_at = Some(1);
    let mut output = [0; 6];
    let error = sample(&failing.filesystem(), &path(), 0, &mut output).unwrap_err();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(Some(&path()), error.path());
    assert_eq!(Some("sampling fixture"), error.provider());
    assert_eq!(2, failing.record().reads);
    assert_eq!(1, failing.record().opens);
    assert_eq!(0, failing.record().path_opens);

    let mut overflow = Config::new(FileSystemLimit::Maximum(2));
    overflow.reported_len = u64::MAX;
    let error = sample(&overflow.filesystem(), &path(), u64::MAX - 1, &mut [0; 2]).unwrap_err();
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    assert_eq!(0, overflow.record().reads);
}

#[test]
fn test_async_sampling_splits_short_reads_and_stops_at_early_eof() {
    let mut config = Config::new(FileSystemLimit::Maximum(3));
    config.short = true;
    let mut output = [0; 8];
    assert_eq!(
        8,
        ready(sample_async(&config.async_filesystem(), &path(), 0, &mut output)).unwrap()
    );
    assert_eq!(*b"abcdefgh", output);
    let record = config.record();
    assert_eq!(1, record.opens);
    assert_eq!(0, record.path_opens);
    assert_eq!(vec![3, 3, 3, 3, 3, 3, 2, 1], record.requests);

    let mut eof = Config::new(FileSystemLimit::Maximum(3));
    eof.bytes = Arc::from(&b"abc"[..]);
    let mut output = [0; 8];
    assert_eq!(
        3,
        ready(sample_async(&eof.async_filesystem(), &path(), 0, &mut output)).unwrap()
    );
    assert_eq!(vec![3, 3], eof.record().requests);
}

#[test]
fn test_async_sampling_handles_empty_zero_nonfinite_error_and_overflow() {
    let zero = Config::new(FileSystemLimit::Maximum(0));
    assert_eq!(
        0,
        ready(sample_async(&zero.async_filesystem(), &path(), 0, &mut [])).unwrap()
    );
    let error = ready(sample_async(&zero.async_filesystem(), &path(), 0, &mut [0])).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(
        0,
        ready(sample_async(&zero.async_filesystem(), &path(), 8, &mut [0])).unwrap()
    );
    assert_eq!(0, zero.record().reads);

    for limit in [
        FileSystemLimit::Unknown,
        FileSystemLimit::Unbounded,
        FileSystemLimit::NotApplicable,
    ] {
        let config = Config::new(limit);
        let mut output = [0; 8];
        assert_eq!(
            8,
            ready(sample_async(&config.async_filesystem(), &path(), 0, &mut output)).unwrap()
        );
        assert_eq!(vec![8], config.record().requests);
    }

    let mut failing = Config::new(FileSystemLimit::Maximum(2));
    failing.error_at = Some(1);
    let error = ready(sample_async(&failing.async_filesystem(), &path(), 0, &mut [0; 6])).unwrap_err();
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(2, failing.record().reads);
    assert_eq!(1, failing.record().opens);

    let mut overflow = Config::new(FileSystemLimit::Maximum(2));
    overflow.reported_len = u64::MAX;
    let error = ready(sample_async(
        &overflow.async_filesystem(),
        &path(),
        u64::MAX - 1,
        &mut [0; 2],
    ))
    .unwrap_err();
    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    assert_eq!(0, overflow.record().reads);
}

#[test]
fn test_async_pending_drop_discards_partial_sample_without_reopening() {
    let mut config = Config::new(FileSystemLimit::Maximum(2));
    config.pending_first = true;
    let filesystem = config.async_filesystem();
    let path = path();
    let mut output = [0xCC; 4];
    {
        let mut future = std::pin::pin!(sample_async(&filesystem, &path, 0, &mut output));
        let mut context = Context::from_waker(Waker::noop());
        assert!(future.as_mut().poll(&mut context).is_pending());
    }
    assert_eq!(b"!\xCC\xCC\xCC", &output);
    assert_eq!(1, config.record().opens);
    assert_eq!(0, config.record().path_opens);
    assert_eq!(1, config.record().reads);
    assert_eq!(vec![2], config.record().requests);
}
