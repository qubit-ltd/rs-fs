// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording providers for public prefix-read policy tests.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncReader;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
#[cfg(feature = "async")]
use qubit_io::AsyncInput;
use qubit_io::Input;

/// Observations shared by both execution modes.
#[derive(Default)]
pub struct Observations {
    pub opens: AtomicUsize,
    pub stats: AtomicUsize,
    pub seen: Mutex<Vec<ReadOptions>>,
    pub read_bytes: AtomicUsize,
}

/// Injects one provider failure without introducing retry behavior.
#[derive(Clone, Copy)]
pub enum Fault {
    None,
    Open,
    Stream,
}

/// Provider exposing fixed bytes and recording the actual dispatched options.
pub struct RecordingProvider {
    pub capabilities: FileSystemCapabilities,
    pub limits: FileSystemLimits,
    pub observations: Arc<Observations>,
    pub fault: Fault,
}

impl RecordingProvider {
    /// Captures the immutable provider contract.
    fn snapshot(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("prefix-test").unwrap(),
                "prefix-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            self.capabilities,
            self.limits,
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }

    /// Records one open and selects bytes as requested by the facade.
    fn open(&self, request: OpenReaderRequest<'_>) -> FsResult<(OpenedFileInfo, RecordingReader)> {
        self.observations.opens.fetch_add(1, Ordering::SeqCst);
        let options = request.options().options().clone();
        self.observations.seen.lock().unwrap().push(options.clone());
        if matches!(self.fault, Fault::Open) {
            return Err(injected_failure());
        }
        let payload = b"abcdefghijklmnop";
        let offset = usize::try_from(options.offset().unwrap_or(0))
            .unwrap_or(usize::MAX)
            .min(payload.len());
        let length = usize::try_from(options.length().unwrap_or(u64::MAX))
            .unwrap_or(usize::MAX)
            .min(payload.len() - offset);
        Ok((
            OpenedFileInfo::new(FileSystemId::new("prefix-test").unwrap(), request.path().clone()),
            RecordingReader {
                bytes: payload[offset..offset + length].to_vec(),
                position: 0,
                observations: Arc::clone(&self.observations),
                fail: matches!(self.fault, Fault::Stream),
            },
        ))
    }

    /// Makes accidental metadata probing observable and unsuccessful.
    fn unexpected_stat(&self) -> FsResult<StatResponse> {
        self.observations.stats.fetch_add(1, Ordering::SeqCst);
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Stat,
            "unexpected stat",
        ))
    }
}

impl FileSystemSpi for RecordingProvider {
    /// Returns the configured snapshot.
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }

    /// Rejects and records unwanted metadata reads.
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        self.unexpected_stat()
    }

    /// Opens the selected synchronous payload once.
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        let (info, reader) = self.open(request)?;
        Ok(OpenedReader::new(info, Box::new(reader)))
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for RecordingProvider {
    /// Returns the same contract in asynchronous mode.
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }

    /// Rejects and records unwanted metadata reads.
    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move { self.unexpected_stat() })
    }

    /// Opens the selected asynchronous payload once.
    fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async move {
            let (info, reader) = self.open(request)?;
            Ok(OpenedAsyncReader::new(info, Box::new(reader)))
        })
    }
}

/// Byte session counting actual bytes consumed by the caller.
struct RecordingReader {
    bytes: Vec<u8>,
    position: usize,
    fail: bool,
    observations: Arc<Observations>,
}

impl RecordingReader {
    /// Copies the next available bytes into a validated output slice.
    fn transfer(&mut self, output: &mut [u8]) -> usize {
        let count = output.len().min(self.bytes.len() - self.position);
        output[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
        self.position += count;
        self.observations.read_bytes.fetch_add(count, Ordering::SeqCst);
        count
    }
}

impl Input for RecordingReader {
    type Item = u8;

    /// Copies bytes within the caller-validated output range.
    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> std::io::Result<usize> {
        if self.fail {
            return Err(injected_failure().into_io_error());
        }
        Ok(self.transfer(&mut output[index..index + count]))
    }
}

#[cfg(feature = "async")]
impl AsyncInput for RecordingReader {
    type Item = u8;

    /// Completes an immediately ready read without a runtime.
    unsafe fn poll_read_unchecked(
        mut self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> std::task::Poll<std::io::Result<usize>> {
        if self.fail {
            return std::task::Poll::Ready(Err(injected_failure().into_io_error()));
        }
        std::task::Poll::Ready(Ok(self.transfer(&mut output[index..index + count])))
    }
}

/// Supplies a typed failure whose underlying source must survive facade
/// wrapping.
fn injected_failure() -> FsError {
    FsError::with_source(
        FsErrorKind::PermissionDenied,
        FsOperation::Read,
        "injected provider failure",
        std::io::Error::other("prefix provider source"),
    )
}
