// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Measures actual facade stream-copy fallback with controlled short I/O.

use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::Mutex;

use criterion::BatchSize;
use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyOptions;
use qubit_fs::error::FsError;
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
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::write::WriteAbortOutcome;
use qubit_io::Input;
use qubit_io::Output;

/// Isolated per-iteration provider with no native copy primitive.
struct CopySpi {
    payload: Arc<[u8]>,
    staging: Mutex<Option<Vec<u8>>>,
    read_chunk: usize,
    write_chunk: usize,
    properties: ProviderProperties,
}

impl CopySpi {
    /// Allocates namespace/session storage outside the timed copy.
    fn new(payload: Arc<[u8]>, read_chunk: usize, write_chunk: usize) -> Self {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("copy-bench").unwrap(),
                "copy-bench",
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
        .unwrap();
        Self {
            staging: Mutex::new(Some(Vec::with_capacity(payload.len()))),
            payload,
            read_chunk,
            write_chunk,
            properties,
        }
    }
}

impl FileSystemSpi for CopySpi {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        if request.path().as_str() != "/source" {
            return Err(FsError::new(
                FsErrorKind::NotFound,
                FsOperation::Stat,
                "no target yet",
            ));
        }
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(self.payload.len() as u64)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(ShortReader {
                payload: Arc::clone(&self.payload),
                position: 0,
                chunk: self.read_chunk,
            }),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(ShortWriter {
                bytes: self
                    .staging
                    .lock()
                    .unwrap()
                    .take()
                    .expect("one writer per iteration"),
                chunk: self.write_chunk,
            }),
        ))
    }
}

/// Reader that exposes deterministic short reads.
struct ShortReader {
    payload: Arc<[u8]>,
    position: usize,
    chunk: usize,
}
impl Input for ShortReader {
    type Item = u8;
    unsafe fn read_unchecked(
        &mut self,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        let count = count
            .min(self.chunk)
            .min(self.payload.len() - self.position);
        output[index..index + count]
            .copy_from_slice(&self.payload[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

/// Preallocated destination accepting deterministic short writes.
struct ShortWriter {
    bytes: Vec<u8>,
    chunk: usize,
}
impl Output for ShortWriter {
    type Item = u8;
    unsafe fn write_unchecked(
        &mut self,
        bytes: &[u8],
        index: usize,
        count: usize,
    ) -> IoResult<usize> {
        let count = count.min(self.chunk);
        self.bytes.extend_from_slice(&bytes[index..index + count]);
        Ok(count)
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}
impl FileWriterSpi for ShortWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        black_box(&self.bytes);
        Ok(
            WriteOutcome::new(AchievedAtomicity::NonAtomic, PublicationMethod::StreamCopy)
                .with_bytes_written(self.bytes.len() as u64),
        )
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}

/// Times transfer and facade policy, keeping fixture allocation outside timing.
fn stream_copy_fallback(c: &mut Criterion) {
    let source = Path::parse("/source").unwrap();
    let target = Path::parse("/target").unwrap();
    let mut group = c.benchmark_group("stream_copy_fallback");
    for size in [0_usize, 1024, 1024 * 1024] {
        let payload: Arc<[u8]> = Arc::from(vec![0xa5; size]);
        for (read_chunk, write_chunk) in [(8192, 8192), (257, 31)] {
            group.throughput(Throughput::Bytes(size as u64));
            group.bench_function(
                BenchmarkId::new(
                    format!("payload-{size}"),
                    format!("read-{read_chunk}-write-{write_chunk}"),
                ),
                |bench| {
                    bench.iter_batched(
                        || {
                            FileSystem::from_spi(CopySpi::new(
                                Arc::clone(&payload),
                                read_chunk,
                                write_chunk,
                            ))
                            .unwrap()
                        },
                        |filesystem| {
                            black_box(
                                filesystem
                                    .copy(&source, &target, CopyOptions::default())
                                    .expect("stream copy succeeds"),
                            )
                        },
                        BatchSize::PerIteration,
                    );
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, stream_copy_fallback);
criterion_main!(benches);
