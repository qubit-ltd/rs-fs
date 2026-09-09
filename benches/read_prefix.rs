// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public facade prefix-read benchmark with a deterministic provider stream.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::Throughput;
use criterion::black_box;
use criterion::criterion_group;
use criterion::criterion_main;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
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
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_io::Input;

struct BenchmarkSpi {
    payload: Arc<[u8]>,
    properties: ProviderProperties,
    consumed: Arc<AtomicUsize>,
    requested_length: Arc<AtomicU64>,
}

impl BenchmarkSpi {
    fn new(payload: Vec<u8>, ranged: bool, consumed: Arc<AtomicUsize>, requested_length: Arc<AtomicU64>) -> Self {
        let mut capabilities = FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read);
        if ranged {
            capabilities = capabilities.with_guaranteed(FileSystemCapability::RangeRead);
        }
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("bench").expect("benchmark id is valid"),
                "bench",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("benchmark properties are valid");
        Self {
            payload: Arc::from(payload.into_boxed_slice()),
            properties,
            consumed,
            requested_length,
        }
    }
}

impl FileSystemSpi for BenchmarkSpi {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(self.payload.len() as u64)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        let options = request.options().options();
        let length = options.length();
        self.requested_length
            .store(length.unwrap_or(u64::MAX), Ordering::Relaxed);
        let offset = usize::try_from(options.offset().unwrap_or(0))
            .unwrap_or(usize::MAX)
            .min(self.payload.len());
        let length = usize::try_from(length.unwrap_or(u64::MAX))
            .unwrap_or(usize::MAX)
            .min(self.payload.len() - offset);
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(BenchmarkReader {
                payload: Arc::clone(&self.payload),
                offset,
                end: offset + length,
                consumed: Arc::clone(&self.consumed),
            }),
        ))
    }
}

/// Shared immutable payload with an independently counted selected window.
struct BenchmarkReader {
    payload: Arc<[u8]>,
    offset: usize,
    end: usize,
    consumed: Arc<AtomicUsize>,
}

impl Input for BenchmarkReader {
    type Item = u8;
    /// Copies at most the provider-selected window into the validated slice.
    unsafe fn read_unchecked(&mut self, output: &mut [u8], index: usize, count: usize) -> std::io::Result<usize> {
        let length = count.min(self.end - self.offset);
        output[index..index + length].copy_from_slice(&self.payload[self.offset..self.offset + length]);
        self.offset += length;
        self.consumed.fetch_add(length, Ordering::Relaxed);
        Ok(length)
    }
}

fn read_prefix(c: &mut Criterion) {
    let path = Path::parse("/payload").expect("benchmark path is valid");
    let mut group = c.benchmark_group("read_prefix");
    for (name, ranged) in [("sequential", false), ("guaranteed-range", true)] {
        for size in [1_usize << 20, 1_usize << 26] {
            let consumed = Arc::new(AtomicUsize::new(0));
            let requested = Arc::new(AtomicU64::new(u64::MAX));
            let filesystem = FileSystem::from_spi(BenchmarkSpi::new(
                vec![0xA5; size],
                ranged,
                Arc::clone(&consumed),
                Arc::clone(&requested),
            ))
            .expect("benchmark facade");
            for maximum in [8192, 65536] {
                consumed.store(0, Ordering::Relaxed);
                let bytes = filesystem
                    .read_prefix(&path, Default::default(), maximum)
                    .expect("benchmark probe");
                assert_eq!(bytes.len(), maximum);
                assert_eq!(consumed.swap(0, Ordering::Relaxed), maximum);
                assert_eq!(
                    requested.load(Ordering::Relaxed),
                    if ranged { maximum as u64 } else { u64::MAX }
                );
                eprintln!(
                    "{name}: payload={size}, prefix={maximum}, result_capacity={}, consumed={}, request_length={:?}",
                    bytes.capacity(),
                    bytes.len(),
                    if ranged { Some(maximum) } else { None }
                );
                group.throughput(Throughput::Bytes(maximum as u64));
                group.bench_with_input(
                    BenchmarkId::new(format!("{name}/payload-{size}"), maximum),
                    &filesystem,
                    |bench, filesystem| {
                        bench.iter(|| {
                            let bytes = filesystem
                                .read_prefix(black_box(&path), Default::default(), maximum)
                                .expect("benchmark prefix");
                            black_box(bytes);
                        });
                    },
                );
            }
        }
    }
    group.finish();
}

criterion_group!(benches, read_prefix);
criterion_main!(benches);
