// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider context preserved by facade-owned copy streams.

use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::io::Error as IoError;
use std::io::ErrorKind as IoErrorKind;
use std::io::Read;
use std::io::Result as IoResult;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyOptions;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
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
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_io::ByteInput;

fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Read,
    Write,
    Flush,
}

struct Spi {
    stage: Stage,
    provider: Option<&'static str>,
    calls: Arc<Mutex<Vec<&'static str>>>,
    bytes: Arc<Mutex<Vec<u8>>>,
}

fn stream_error(provider: Option<&str>) -> IoError {
    let mut error = FsError::with_source(
        FsErrorKind::PermissionDenied,
        FsOperation::Write,
        "stream failure",
        Marker,
    )
    .with_effect_state(FsEffectState::Unchanged);
    if let Some(provider) = provider {
        error = error.with_provider(provider);
    }
    IoError::new(IoErrorKind::PermissionDenied, error)
}

#[derive(Debug)]
struct Marker;

impl Display for Marker {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("provider marker")
    }
}

impl Error for Marker {}

impl FileSystemSpi for Spi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("provider-context").unwrap(),
                "facade-provider",
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
        .unwrap()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls.lock().unwrap().push("stat");
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(3)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.calls.lock().unwrap().push("open_reader");
        let fail = matches!(self.stage, Stage::Read);
        let provider = self.provider;
        let reader: Box<dyn ByteInput + Send> = Box::new(Reader {
            fail,
            provider,
            position: 0,
        });
        Ok(OpenedReader::new(info(request.path()), reader))
    }

    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.calls.lock().unwrap().push("open_writer");
        let writer = Writer {
            stage: self.stage,
            provider: self.provider,
            bytes: Arc::clone(&self.bytes),
            calls: Arc::clone(&self.calls),
        };
        Ok(OpenedWriter::new(info(request.path()), Box::new(writer)))
    }
}

fn info(path: &Path) -> OpenedFileInfo {
    OpenedFileInfo::new(FileSystemId::new("provider-context").unwrap(), path.clone())
}

struct Reader {
    fail: bool,
    provider: Option<&'static str>,
    position: usize,
}

impl Read for Reader {
    fn read(&mut self, buffer: &mut [u8]) -> IoResult<usize> {
        if self.fail {
            return Err(stream_error(self.provider));
        }
        if self.position == 3 {
            return Ok(0);
        }
        let read = buffer.len().min(3);
        buffer[..read].copy_from_slice(&b"abc"[..read]);
        self.position += read;
        Ok(read)
    }
}

struct Writer {
    stage: Stage,
    provider: Option<&'static str>,
    bytes: Arc<Mutex<Vec<u8>>>,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl Write for Writer {
    fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
        if matches!(self.stage, Stage::Write) {
            return Err(stream_error(self.provider));
        }
        self.bytes.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        if matches!(self.stage, Stage::Flush) {
            Err(stream_error(self.provider))
        } else {
            Ok(())
        }
    }
}

impl FileWriterSpi for Writer {
    fn commit(&mut self) -> Result<WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
        self.calls.lock().unwrap().push("commit");
        Ok(WriteOutcome::new(
            AchievedAtomicity::NonAtomic,
            PublicationMethod::StreamCopy,
        ))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}

fn check(stage: Stage, provider: Option<&'static str>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let fs = FileSystem::from_spi(Spi {
        stage,
        provider,
        calls: Arc::clone(&calls),
        bytes: Arc::clone(&bytes),
    })
    .unwrap();
    let failure = fs
        .copy(&path("/source"), &path("/target"), CopyOptions::default())
        .unwrap_err();
    let error = failure.error();
    assert_eq!(FsOperation::Copy, error.operation());
    assert_eq!(FsErrorKind::PermissionDenied, error.kind());
    assert_eq!(Some(FsEffectState::Unchanged), error.effect_state());
    assert_eq!(
        Some(provider.unwrap_or("facade-provider")),
        error.provider(),
        "stage={stage:?}, input provider={provider:?}"
    );
    let expected_path = if matches!(stage, Stage::Read) {
        "/source"
    } else {
        "/target"
    };
    assert_eq!(Some(&path(expected_path)), error.path());
    assert_eq!(Some(&path("/target")), error.target());
    let mut source = Error::source(error);
    let mut found_marker = false;
    while let Some(cause) = source {
        if cause.downcast_ref::<Marker>().is_some() {
            found_marker = true;
        }
        source = cause.source();
    }
    assert!(found_marker, "source marker must remain available in the cause chain");
    let expected_bytes = if matches!(stage, Stage::Flush) { 3 } else { 0 };
    let expected_state = CopyFailureState::Indeterminate;
    assert_eq!(expected_state, failure.state(), "stage={stage:?}");
    assert_eq!(expected_bytes, failure.partial_stats().bytes);
    assert!(failure.has_recovery());
    assert_eq!(expected_bytes as usize, bytes.lock().unwrap().len());
    let calls = calls.lock().unwrap();
    assert!(!calls.contains(&"try_copy"));
    assert_eq!(1, calls.iter().filter(|call| **call == "open_writer").count());
    assert_eq!(0, calls.iter().filter(|call| **call == "commit").count());
}

#[test]
fn test_provider_context_sync_read_write_flush_preserves_or_fills_provider() {
    for stage in [Stage::Read, Stage::Write, Stage::Flush] {
        for provider in [Some("stream-provider"), None] {
            check(stage, provider);
        }
    }
}

#[cfg(feature = "async")]
mod asynchronous {
    use std::error::Error;
    use std::future::Future;
    use std::io::Result as IoResult;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::task::Context;
    use std::task::Poll;
    use std::task::Waker;

    use qubit_fs::AsyncFileSystem;
    use qubit_fs::FsResult;
    use qubit_fs::Path;
    use qubit_fs::copy::CopyFailureState;
    use qubit_fs::copy::CopyOptions;
    use qubit_fs::error::FsEffectState;
    use qubit_fs::error::FsErrorKind;
    use qubit_fs::error::FsOperation;
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
    use qubit_fs::spi::AsyncFileSystemSpi;
    use qubit_fs::spi::AsyncFileWriteSession;
    use qubit_fs::spi::OpenReaderRequest;
    use qubit_fs::spi::OpenWriterRequest;
    use qubit_fs::spi::OpenedAsyncReader;
    use qubit_fs::spi::OpenedAsyncWriter;
    use qubit_fs::spi::ProviderOperation;
    use qubit_fs::spi::ProviderOperations;
    use qubit_fs::spi::ProviderProperties;
    use qubit_fs::spi::SpiFuture;
    use qubit_fs::spi::StatRequest;
    use qubit_fs::spi::StatResponse;
    use qubit_fs::write::WriteAbortOutcome;
    use qubit_fs::write::WriteFailure;
    use qubit_io::AsyncInput;
    use qubit_io::AsyncOutput;

    use super::Marker;
    use super::Stage;
    use super::path;
    use super::stream_error;

    struct AsyncSpi {
        stage: Stage,
        provider: Option<&'static str>,
        calls: Arc<Mutex<Vec<&'static str>>>,
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    fn async_info(path: &Path) -> OpenedFileInfo {
        OpenedFileInfo::new(FileSystemId::new("async-provider-context").unwrap(), path.clone())
    }

    impl AsyncFileSystemSpi for AsyncSpi {
        fn properties(&self) -> ProviderProperties {
            ProviderProperties::new(
                FileSystemInfo::new(
                    FileSystemId::new("async-provider-context").unwrap(),
                    "facade-provider",
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
            .unwrap()
        }

        fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            self.calls.lock().unwrap().push("stat");
            Box::pin(async move {
                Ok(StatResponse::new(
                    request.path().clone(),
                    FileMetadata::new(FileKind::File).with_len(Some(3)),
                ))
            })
        }

        fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
            self.calls.lock().unwrap().push("open_reader");
            let reader = AsyncReader {
                fail: matches!(self.stage, Stage::Read),
                provider: self.provider,
                position: 0,
            };
            Box::pin(async move { Ok(OpenedAsyncReader::new(async_info(request.path()), Box::new(reader))) })
        }

        fn open_writer<'a>(&'a self, request: OpenWriterRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncWriter>> {
            self.calls.lock().unwrap().push("open_writer");
            let writer = AsyncWriter {
                stage: self.stage,
                provider: self.provider,
                bytes: Arc::clone(&self.bytes),
                calls: Arc::clone(&self.calls),
            };
            Box::pin(async move { Ok(OpenedAsyncWriter::new(async_info(request.path()), Box::new(writer))) })
        }
    }

    struct AsyncReader {
        fail: bool,
        provider: Option<&'static str>,
        position: usize,
    }
    impl AsyncInput for AsyncReader {
        type Item = u8;
        unsafe fn poll_read_unchecked(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            output: &mut [u8],
            index: usize,
            count: usize,
        ) -> Poll<IoResult<usize>> {
            let this = self.as_mut().get_mut();
            if this.fail {
                return Poll::Ready(Err(stream_error(this.provider)));
            }
            if this.position == 3 {
                return Poll::Ready(Ok(0));
            }
            let read = count.min(3);
            output[index..index + read].copy_from_slice(&b"abc"[..read]);
            this.position += read;
            Poll::Ready(Ok(read))
        }
    }

    struct AsyncWriter {
        stage: Stage,
        provider: Option<&'static str>,
        bytes: Arc<Mutex<Vec<u8>>>,
        calls: Arc<Mutex<Vec<&'static str>>>,
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
            if matches!(this.stage, Stage::Write) {
                return Poll::Ready(Err(stream_error(this.provider)));
            }
            this.bytes
                .lock()
                .unwrap()
                .extend_from_slice(&input[index..index + count]);
            Poll::Ready(Ok(count))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
            let this = self.get_mut();
            if matches!(this.stage, Stage::Flush) {
                Poll::Ready(Err(stream_error(this.provider)))
            } else {
                Poll::Ready(Ok(()))
            }
        }
    }

    impl AsyncFileWriteSession for AsyncWriter {
        fn commit_async<'a>(mut self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<WriteOutcome, WriteFailure>> {
            self.as_mut().get_mut().calls.lock().unwrap().push("commit");
            Box::pin(async {
                Ok(WriteOutcome::new(
                    AchievedAtomicity::NonAtomic,
                    PublicationMethod::StreamCopy,
                ))
            })
        }
        fn abort_async<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<WriteAbortOutcome>> {
            Box::pin(async { Ok(WriteAbortOutcome::NotPublished) })
        }
    }

    fn run<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
                return output;
            }
        }
    }

    fn check(stage: Stage, provider: Option<&'static str>) {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let fs = AsyncFileSystem::from_spi(AsyncSpi {
            stage,
            provider,
            calls: Arc::clone(&calls),
            bytes: Arc::clone(&bytes),
        })
        .unwrap();
        let mut operation = fs
            .begin_copy(path("/source"), path("/target"), CopyOptions::default())
            .unwrap();
        let failure = run(operation.execute()).unwrap_err();
        let error = failure.error();
        assert_eq!(FsOperation::Copy, error.operation());
        assert_eq!(FsErrorKind::PermissionDenied, error.kind());
        assert_eq!(Some(FsEffectState::Unchanged), error.effect_state());
        assert_eq!(
            Some(provider.unwrap_or("facade-provider")),
            error.provider(),
            "stage={stage:?}, provider={provider:?}"
        );
        let expected_path = if matches!(stage, Stage::Read) {
            "/source"
        } else {
            "/target"
        };
        assert_eq!(Some(&path(expected_path)), error.path());
        assert_eq!(Some(&path("/target")), error.target());
        let mut source = Error::source(error);
        let mut found_marker = false;
        while let Some(cause) = source {
            if cause.downcast_ref::<Marker>().is_some() {
                found_marker = true;
            }
            source = cause.source();
        }
        assert!(found_marker);
        let expected_bytes = if matches!(stage, Stage::Flush) { 3 } else { 0 };
        assert_eq!(CopyFailureState::Indeterminate, failure.state());
        assert_eq!(expected_bytes, failure.partial_stats().bytes);
        assert!(operation.has_recovery());
        assert_eq!(expected_bytes as usize, bytes.lock().unwrap().len());
        let calls = calls.lock().unwrap();
        assert!(!calls.contains(&"try_copy"));
        assert_eq!(1, calls.iter().filter(|call| **call == "open_writer").count());
        assert_eq!(0, calls.iter().filter(|call| **call == "commit").count());
    }

    #[test]
    fn test_provider_context_async_read_write_flush_preserves_or_fills_provider() {
        for stage in [Stage::Read, Stage::Write, Stage::Flush] {
            for provider in [Some("stream-provider"), None] {
                check(stage, provider);
            }
        }
    }
}
