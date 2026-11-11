// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Samples caller-owned storage asynchronously from one opened session.
//!
//! A finite provider range limit bounds each read request, not the sample's
//! total size. Errors or cancellation can follow partial writes, so discard
//! the output and do not reopen the path.
use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::read::AsyncRandomAccessReader;
use qubit_fs::read::ReadOptions;

/// Reads a sample in provider-sized ranges, retaining one opened session.
///
/// A short positive read advances to the next offset. A zero read ends the
/// sample and returns the number of bytes filled. `Unknown` does not guarantee
/// that a backend accepts an arbitrarily large request. If this future is
/// cancelled or returns an error, discard `output`; the buffer may contain
/// partial data and this helper does not retry or reopen the path.
pub async fn sample_async(
    filesystem: &AsyncFileSystem,
    path: &Path,
    offset: u64,
    output: &mut [u8],
) -> FsResult<usize> {
    let limit = filesystem.properties().limits().max_read_range_bytes();
    let mut reader: AsyncRandomAccessReader = filesystem
        .open_random_access_reader(path, ReadOptions::default())
        .await?;
    let mut position = offset;
    let mut filled = 0;
    if output.is_empty() || position >= reader.len() {
        return Ok(0);
    }
    let output_len = u64::try_from(output.len()).map_err(|_| {
        FsError::new(
            FsErrorKind::InvalidOptions,
            FsOperation::Read,
            "sample length cannot be represented",
        )
        .with_path(path.clone())
        .with_provider(filesystem.properties().info().provider_id())
    })?;
    position.checked_add(output_len).ok_or_else(|| {
        FsError::new(FsErrorKind::InvalidOptions, FsOperation::Read, "sample range overflows")
            .with_path(path.clone())
            .with_provider(filesystem.properties().info().provider_id())
    })?;
    let segment_limit = match limit {
        FileSystemLimit::Maximum(maximum) => usize::try_from(maximum).unwrap_or(usize::MAX),
        FileSystemLimit::Unknown | FileSystemLimit::Unbounded | FileSystemLimit::NotApplicable => usize::MAX,
    };
    if segment_limit == 0 {
        return Err(FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::Read,
            "random-access sample cannot progress with a zero range limit",
        )
        .with_path(path.clone())
        .with_provider(filesystem.properties().info().provider_id()));
    }
    while filled < output.len() && position < reader.len() {
        let request_len = (output.len() - filled).min(segment_limit);
        let count = reader
            .read_at(position, &mut output[filled..filled + request_len])
            .await?;
        if count == 0 {
            break;
        }
        let count_u64 = u64::try_from(count).map_err(|_| {
            FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Read,
                "read count cannot be represented",
            )
            .with_path(path.clone())
            .with_provider(filesystem.properties().info().provider_id())
        })?;
        position = position.checked_add(count_u64).ok_or_else(|| {
            FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Read,
                "sample offset overflows",
            )
            .with_path(path.clone())
            .with_provider(filesystem.properties().info().provider_id())
        })?;
        filled += count;
    }
    Ok(filled)
}
