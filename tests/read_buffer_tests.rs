// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Aggregate reads must grow buffers from consumed bytes, not metadata hints.

#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;

use std::io::Cursor;
#[cfg(feature = "async")]
use std::pin::Pin;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
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

struct MetadataProvider {
    hint: Option<u64>,
    length: usize,
}

impl MetadataProvider {
    /// Declares sequential reading without metadata probes.
    fn snapshot(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("buffer").expect("id"),
                "buffer",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("properties")
    }

    /// Captures potentially outdated full-resource metadata.
    fn info(&self, path: &Path) -> OpenedFileInfo {
        OpenedFileInfo::new(FileSystemId::new("buffer").expect("id"), path.clone())
            .with_metadata(FileMetadata::new(FileKind::File).with_len(self.hint))
    }
}

impl FileSystemSpi for MetadataProvider {
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(unexpected_stat())
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            self.info(request.path()),
            Box::new(Cursor::new(vec![7; self.length])),
        ))
    }
}

/// Metadata must come from open, never from an extra stat request.
fn unexpected_stat() -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::Stat,
        "unexpected stat",
    )
}

#[cfg(feature = "async")]
struct ReadyReader(Cursor<Vec<u8>>);

#[cfg(feature = "async")]
impl AsyncInput for ReadyReader {
    type Item = u8;
    unsafe fn poll_read_unchecked(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        out: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<std::io::Result<usize>> {
        Poll::Ready(std::io::Read::read(&mut self.0, &mut out[index..index + count]))
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for MetadataProvider {
    fn properties(&self) -> ProviderProperties {
        self.snapshot()
    }
    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { Err(unexpected_stat()) })
    }
    fn open_reader<'a>(&'a self, request: OpenReaderRequest<'a>) -> SpiFuture<'a, FsResult<OpenedAsyncReader>> {
        Box::pin(async move {
            Ok(OpenedAsyncReader::new(
                self.info(request.path()),
                Box::new(ReadyReader(Cursor::new(vec![7; self.length]))),
            ))
        })
    }
}

/// An enormous stale length must not force a huge allocation for a short
/// stream.
#[test]
fn test_read_all_does_not_preallocate_entire_metadata_length() {
    let fs = FileSystem::from_spi(MetadataProvider {
        hint: Some(usize::MAX as u64),
        length: 3,
    })
    .expect("filesystem");
    let bytes = fs
        .read_all(
            &Path::parse("/payload").expect("path"),
            ReadOptions::default(),
            usize::MAX,
        )
        .expect("bounded incremental allocation");
    assert_eq!(bytes, [7; 3]);
}

/// Async readers use exactly the same incremental allocation policy.
#[cfg(feature = "async")]
#[test]
fn test_async_read_all_does_not_preallocate_entire_metadata_length() {
    let fs = AsyncFileSystem::from_spi(MetadataProvider {
        hint: Some(usize::MAX as u64),
        length: 3,
    })
    .expect("filesystem");
    let path = Path::parse("/payload").expect("path");
    let bytes =
        poll_support::ready(fs.read_all(&path, ReadOptions::default(), usize::MAX)).expect("incremental allocation");
    assert_eq!(bytes, [7; 3]);
}

/// Unknown and underestimated lengths still enforce the actual byte limit.
#[test]
fn test_unknown_and_low_metadata_enforce_actual_bytes() {
    let path = Path::parse("/payload").expect("path");
    for hint in [None, Some(1)] {
        let fs = FileSystem::from_spi(MetadataProvider { hint, length: 20_000 }).expect("filesystem");
        assert_eq!(
            fs.read_all(&path, ReadOptions::default(), 20_000)
                .expect("exact limit")
                .len(),
            20_000
        );
        let error = fs
            .read_all(&path, ReadOptions::default(), 19_999)
            .expect_err("over limit");
        assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
        assert_eq!(error.path(), Some(&path));
        assert_eq!(
            fs.read_prefix(&path, ReadOptions::default(), 19_999)
                .expect("prefix")
                .len(),
            19_999
        );
    }
}
