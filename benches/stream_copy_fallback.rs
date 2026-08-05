// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public facade prefix-read benchmark with a deterministic provider stream.

use std::{
    io::Cursor,
    sync::Arc,
};

use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    black_box,
    criterion_group,
    criterion_main,
};
use qubit_fs::spi::{
    FileSystemSpi,
    OpenReaderRequest,
    OpenedReader,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    FileKind,
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsResult,
    OpenedFileInfo,
    Path,
    PathConstraints,
    PathSemantics,
    SymlinkPolicy,
};

struct BenchmarkSpi {
    payload: Arc<Vec<u8>>,
    properties: FileSystemProperties,
}

impl BenchmarkSpi {
    fn new(payload: Vec<u8>) -> Self {
        let properties = FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("bench").expect("benchmark id is valid"),
                "bench",
                PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with(qubit_fs::FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("benchmark properties are valid");
        Self {
            payload: Arc::new(payload),
            properties,
        }
    }
}

impl FileSystemSpi for BenchmarkSpi {
    fn properties(&self) -> FileSystemProperties {
        self.properties.clone()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File)
                .with_len(Some(self.payload.len() as u64)),
        ))
    }

    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                self.properties.info().id().clone(),
                request.path().clone(),
            ),
            Box::new(Cursor::new(self.payload.as_ref().clone())),
        ))
    }
}

fn stream_copy_fallback(c: &mut Criterion) {
    let path = Path::parse("/payload").expect("benchmark path is valid");
    let mut group = c.benchmark_group("facade_read_prefix");
    for size in [1_usize << 10, 1_usize << 20, 1_usize << 26] {
        let filesystem =
            FileSystem::from_spi(BenchmarkSpi::new(vec![0xA5; size]))
                .expect("benchmark facade should construct");
        group.throughput(Throughput::Bytes(size as u64));
        for max_bytes in [8 * 1024, 64 * 1024, 1024 * 1024] {
            group.bench_with_input(
                BenchmarkId::new("prefix", max_bytes),
                &filesystem,
                |bench, filesystem| {
                    bench.iter(|| {
                        let bytes = filesystem
                            .read_prefix(
                                black_box(&path),
                                Default::default(),
                                max_bytes,
                            )
                            .expect("benchmark prefix read should succeed");
                        black_box(bytes.len());
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, stream_copy_fallback);
criterion_main!(benches);
