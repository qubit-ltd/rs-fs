// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Samples caller-owned storage from one opened random-access session.
//!
//! A finite provider range limit bounds each read request, not the sample's
//! total size. Errors can follow partial writes, so callers must discard the
//! output on error. This helper never retries or reopens the path.
use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::read::RandomAccessReader;

/// Reads a sample in provider-sized ranges, retaining one opened session.
///
/// `reader` must have been opened by `filesystem`, which supplies this sample's
/// configured limits and provider context.
///
/// A short positive read advances to the next offset. A zero read ends the
/// sample and returns the number of bytes filled; it does not claim the output
/// was filled. `Unknown` does not guarantee that a backend accepts an
/// arbitrarily large request. On any error, discard `output` because it may
/// contain bytes from earlier or partially completed reads.
pub fn sample(
    filesystem: &FileSystem,
    reader: &mut RandomAccessReader,
    offset: u64,
    output: &mut [u8],
) -> FsResult<usize> {
    let path = reader.info().path().clone();
    let limit = filesystem.properties().limits().max_read_range_bytes();
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
        let count = reader.read_at(position, &mut output[filled..filled + request_len])?;
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
