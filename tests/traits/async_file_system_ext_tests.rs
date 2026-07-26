// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
use std::future::Future;
use std::io::{
    Error as IoError,
    ErrorKind as IoErrorKind,
    Result as IoResult,
};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{
    AtomicUsize,
    Ordering,
};
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AchievedAtomicity,
    AsyncFileReader,
    AsyncFileSystem,
    AsyncFileSystemExt,
    AsyncFileWriteSession,
    AsyncFileWriter,
    FileKind,
    FileLocation,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsPath,
    OpenedFileInfo,
    PathSemantics,
    PublicationMethod,
    ReadOptions,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::{
    AsyncInput,
    AsyncOutput,
};

#[derive(Clone, Copy)]
enum ExtMode {
    EmptyRead,
    InterruptedRead,
    ProbeErrorRead,
    TimedOutRead,
    EmbeddedFsErrorRead,
    ReadError,
    OpenReaderError,
    WriteError,
    OpenWriterError,
    CommitError,
}

struct ExtAsyncFs {
    info: FileSystemInfo,
    limits: FileSystemLimits,
    mode: ExtMode,
    aborts: Arc<AtomicUsize>,
}

impl ExtAsyncFs {
    fn new(mode: ExtMode) -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new("async-ext").unwrap(),
                "async-ext",
                PathSemantics::Hierarchical,
            ),
            limits: FileSystemLimits::unknown(),
            mode,
            aborts: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn with_limits(mut self, limits: FileSystemLimits) -> Self {
        self.limits = limits;
        self
    }
}

impl FileSystemProperties for ExtAsyncFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

impl AsyncFileSystem for ExtAsyncFs {
    fn stat_async<'a>(
        &'a self,
        _path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata> {
        Box::pin(async { Ok(FileMetadata::new(FileKind::File)) })
    }

    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader> {
        let result = match self.mode {
            ExtMode::OpenReaderError => Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::OpenReader,
                "open reader failed",
            )),
            ExtMode::EmptyRead => {
                Ok(AsyncFileReader::new(ExtInput::Eof, opened_info(path)))
            }
            ExtMode::InterruptedRead => Ok(AsyncFileReader::new(
                ExtInput::Interrupted,
                opened_info(path),
            )),
            ExtMode::ProbeErrorRead => Ok(AsyncFileReader::new(
                ExtInput::ProbeError { emitted: false },
                opened_info(path),
            )),
            ExtMode::TimedOutRead => {
                Ok(AsyncFileReader::new(ExtInput::TimedOut, opened_info(path)))
            }
            ExtMode::EmbeddedFsErrorRead => Ok(AsyncFileReader::new(
                ExtInput::EmbeddedFsError,
                opened_info(path),
            )),
            _ => Ok(AsyncFileReader::new(ExtInput::Error, opened_info(path))),
        };
        Box::pin(async move { result })
    }

    fn open_writer_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: WriteOptions,
    ) -> FsFuture<'a, AsyncFileWriter> {
        if matches!(self.mode, ExtMode::OpenWriterError) {
            return Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::OpenWriter,
                    "open writer failed",
                ))
            });
        }
        let session = ExtWriteSession {
            fail_write: matches!(self.mode, ExtMode::WriteError),
            fail_commit: matches!(self.mode, ExtMode::CommitError),
            aborts: self.aborts.clone(),
        };
        let info = opened_info(path);
        Box::pin(async move { Ok(AsyncFileWriter::new(session, info)) })
    }
}

enum ExtInput {
    Interrupted,
    ProbeError { emitted: bool },
    TimedOut,
    EmbeddedFsError,
    Error,
    Eof,
}

impl AsyncInput for ExtInput {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        match this {
            Self::Interrupted => {
                *this = Self::Eof;
                Poll::Ready(Err(IoError::from(IoErrorKind::Interrupted)))
            }
            Self::ProbeError { emitted } if *emitted => {
                Poll::Ready(Err(IoError::other("probe failed")))
            }
            Self::ProbeError { emitted } if count > 0 => {
                output[index] = b'x';
                *emitted = true;
                Poll::Ready(Ok(1))
            }
            Self::ProbeError { .. } => Poll::Ready(Ok(0)),
            Self::TimedOut => {
                Poll::Ready(Err(IoError::from(IoErrorKind::TimedOut)))
            }
            Self::EmbeddedFsError => Poll::Ready(Err(FsError::new(
                FsErrorKind::QuotaExceeded,
                FsOperation::OpenReader,
                "provider quota exhausted",
            )
            .with_provider("stream-provider")
            .into_io_error())),
            Self::Error => Poll::Ready(Err(IoError::other("read failed"))),
            Self::Eof => Poll::Ready(Ok(0)),
        }
    }
}

struct ExtWriteSession {
    fail_write: bool,
    fail_commit: bool,
    aborts: Arc<AtomicUsize>,
}

impl AsyncOutput for ExtWriteSession {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _input: &[u8],
        _index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        if self.fail_write {
            Poll::Ready(Err(IoError::other("write failed")))
        } else {
            Poll::Ready(Ok(count))
        }
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for ExtWriteSession {
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, WriteOutcome> {
        if self.fail_commit {
            Box::pin(async {
                Err(FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CommitWriter,
                    "commit failed",
                ))
            })
        } else {
            Box::pin(async {
                Ok(WriteOutcome::new(
                    AchievedAtomicity::Atomic,
                    PublicationMethod::Direct,
                ))
            })
        }
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        self.aborts.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(()) })
    }
}

fn opened_info(path: &FsPath) -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("async-ext").unwrap(),
        path.clone(),
    ))
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

#[test]
fn async_file_system_extensions_reject_interrupted_reads() {
    let fs = ExtAsyncFs::new(ExtMode::InterruptedRead);
    let path = FsPath::parse("/file").unwrap();

    let error = ready(fs.read_all_async(&path, 1))
        .expect_err("reject Interrupted across the asynchronous I/O boundary");
    let source = Error::source(&error)
        .and_then(|source| source.downcast_ref::<IoError>())
        .expect("retain asynchronous I/O contract error");

    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(IoErrorKind::InvalidData, source.kind());
}

#[test]
fn async_read_all_accepts_an_empty_resource() {
    let fs = ExtAsyncFs::new(ExtMode::EmptyRead);
    let path = FsPath::parse("/empty").unwrap();

    assert!(ready(fs.read_all_async(&path, 1)).unwrap().is_empty());
}

#[test]
fn async_read_all_preserves_probe_errors() {
    let path = FsPath::parse("/probe-error").unwrap();
    let error = ready(
        ExtAsyncFs::new(ExtMode::ProbeErrorRead).read_all_async(&path, 1),
    )
    .unwrap_err();

    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn async_read_all_preserves_specific_standard_io_error_kinds() {
    let path = FsPath::parse("/timeout").expect("path should parse");

    let error =
        ready(ExtAsyncFs::new(ExtMode::TimedOutRead).read_all_async(&path, 1))
            .expect_err("timed-out stream should fail");

    assert_eq!(FsErrorKind::Timeout, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn async_read_all_restores_embedded_file_system_error_context() {
    let path = FsPath::parse("/quota").expect("path should parse");

    let error = ready(
        ExtAsyncFs::new(ExtMode::EmbeddedFsErrorRead).read_all_async(&path, 1),
    )
    .expect_err("quota error should fail the read");

    assert_eq!(FsErrorKind::QuotaExceeded, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("stream-provider"), error.provider());
}

#[test]
fn async_extension_methods_preflight_provider_write_limits() {
    let path = FsPath::parse("/file").unwrap();
    let fs = ExtAsyncFs::new(ExtMode::InterruptedRead).with_limits(
        FileSystemLimits::unknown()
            .with_max_write_bytes(FileSystemLimit::Maximum(3)),
    );

    let error = ready(fs.write_all_async(&path, b"data")).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(FsOperation::Write, error.operation());
    assert_eq!(Some(&path), error.path());
}

#[test]
fn async_extension_methods_preflight_provider_path_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(3));
    let path = FsPath::parse("/file").unwrap();

    let read_error = ready(
        ExtAsyncFs::new(ExtMode::InterruptedRead)
            .with_limits(limits)
            .read_all_async(&path, 4),
    )
    .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, read_error.kind());
    assert_eq!(FsOperation::Read, read_error.operation());

    let write_error = ready(
        ExtAsyncFs::new(ExtMode::InterruptedRead)
            .with_limits(limits)
            .write_all_async(&path, b"data"),
    )
    .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, write_error.kind());
    assert_eq!(FsOperation::Write, write_error.operation());
}

#[test]
fn async_file_system_extensions_preserve_open_and_transfer_errors() {
    let path = FsPath::parse("/file").unwrap();

    let error = ready(
        ExtAsyncFs::new(ExtMode::OpenReaderError).read_all_async(&path, 16),
    )
    .unwrap_err();
    assert_eq!(FsOperation::OpenReader, error.operation());

    let error =
        ready(ExtAsyncFs::new(ExtMode::ReadError).read_all_async(&path, 16))
            .unwrap_err();
    assert_eq!(FsOperation::Read, error.operation());

    let error = ready(
        ExtAsyncFs::new(ExtMode::OpenWriterError)
            .write_all_async(&path, b"data"),
    )
    .unwrap_err();
    assert_eq!(FsOperation::OpenWriter, error.operation());

    let fs = ExtAsyncFs::new(ExtMode::WriteError);
    let error = ready(fs.write_all_async(&path, b"data")).unwrap_err();
    assert_eq!(FsOperation::Write, error.operation());
    assert_eq!(1, fs.aborts.load(Ordering::Relaxed));

    let error = ready(
        ExtAsyncFs::new(ExtMode::CommitError).write_all_async(&path, b"data"),
    )
    .unwrap_err();
    assert_eq!(FsOperation::CommitWriter, error.operation());
}
