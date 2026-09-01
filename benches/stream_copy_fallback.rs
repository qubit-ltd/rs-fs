// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public facade prefix-read benchmark with a deterministic provider stream.

use std::io::Cursor;
use std::sync::Arc;

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

struct BenchmarkSpi {
    payload: Arc<[u8]>,
    properties: ProviderProperties,
}

impl BenchmarkSpi {
    fn new(payload: Vec<u8>) -> Self {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("bench").expect("benchmark id is valid"),
                "bench",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(qubit_fs::metadata::FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("benchmark properties are valid");
        Self {
            payload: Arc::from(payload.into_boxed_slice()),
            properties,
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
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(Arc::clone(&self.payload))),
        ))
    }
}

fn read_prefix(c: &mut Criterion) {
    let path = Path::parse("/payload").expect("benchmark path is valid");
    let mut group = c.benchmark_group("read_prefix");
    for size in [1_usize << 10, 1_usize << 20, 1_usize << 26] {
        let filesystem =
            FileSystem::from_spi(BenchmarkSpi::new(vec![0xA5; size])).expect("benchmark facade should construct");
        for max_bytes in [8 * 1024, 64 * 1024, 1024 * 1024] {
            group.throughput(Throughput::Bytes(size.min(max_bytes) as u64));
            group.bench_with_input(
                BenchmarkId::new("prefix", max_bytes),
                &filesystem,
                |bench, filesystem| {
                    bench.iter(|| {
                        let bytes = filesystem
                            .read_prefix(black_box(&path), Default::default(), max_bytes)
                            .expect("benchmark prefix read should succeed");
                        black_box(bytes.len());
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, read_prefix);
criterion_main!(benches);
