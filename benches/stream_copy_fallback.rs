// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Baseline measurements for the synchronous stream-copy fallback loop.

use std::io::{
    Cursor,
    Read,
    Write,
};
use std::time::Duration;

use criterion::{
    BenchmarkId,
    Criterion,
    Throughput,
    black_box,
    criterion_group,
    criterion_main,
};

/// Copies bytes with the fallback's current fixed 8 KiB transfer buffer.
fn copy_with_buffer(input: &[u8], buffer_size: usize) -> usize {
    let mut reader = Cursor::new(input);
    let mut writer = Vec::with_capacity(input.len());
    let mut buffer = vec![0_u8; buffer_size];
    loop {
        let read = reader
            .read(&mut buffer)
            .expect("in-memory benchmark read should succeed");
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .expect("in-memory benchmark write should succeed");
    }
    writer.len()
}

/// Measures representative payload sizes and candidate buffer sizes before
/// changing the production fallback loop.
fn stream_copy_fallback(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_copy_fallback");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(500));
    for size in [1_usize << 10, 1_usize << 20, 1_usize << 26] {
        let input = vec![0xA5_u8; size];
        group.throughput(Throughput::Bytes(size as u64));
        for buffer_size in [8 * 1024, 64 * 1024, 1024 * 1024] {
            group.bench_with_input(
                BenchmarkId::new("buffer", buffer_size),
                &input,
                |bench, input| {
                    bench.iter(|| {
                        black_box(copy_with_buffer(
                            black_box(input),
                            buffer_size,
                        ))
                    });
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, stream_copy_fallback);
criterion_main!(benches);
