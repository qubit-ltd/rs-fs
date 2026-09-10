// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Range validation must precede provider dispatch and prefix optimization.

#[cfg(feature = "async")]
#[path = "common/poll_support.rs"]
mod poll_support;
#[allow(dead_code)] // This suite uses range behaviors, not the support fixture's stream failures.
#[path = "support/prefix_read.rs"]
mod prefix_read_support;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use prefix_read_support::Fault;
use prefix_read_support::Observations;
use prefix_read_support::RecordingProvider;
#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::read::ReadOptions;

/// Creates a recording provider with optional range support.
fn provider(observations: &Arc<Observations>, ranged: bool) -> RecordingProvider {
    let capabilities = FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read);
    RecordingProvider {
        capabilities: if ranged {
            capabilities.with_guaranteed(FileSystemCapability::RangeRead)
        } else {
            capabilities
        },
        observations: Arc::clone(observations),
        limits: FileSystemLimits::unknown(),
        fault: Fault::None,
    }
}

/// Even a prefix that could fit must not hide the original range overflow.
#[test]
fn test_range_overflow_precedes_sync_dispatch_and_capabilities() {
    for ranged in [false, true] {
        let observations = Arc::new(Observations::default());
        let fs = FileSystem::from_spi(provider(&observations, ranged)).expect("filesystem");
        let path = Path::parse("/payload").expect("path");
        let options = ReadOptions::default().with_offset(Some(u64::MAX)).with_length(Some(1));
        let errors = [
            fs.open_reader(&path, options.clone()).expect_err("overflow"),
            fs.read_prefix(&path, options, 0)
                .expect_err("overflow before narrowing"),
        ];
        for error in errors {
            assert_eq!(error.kind(), FsErrorKind::InvalidOptions);
            assert_eq!(error.operation(), FsOperation::OpenReader);
            assert_eq!(error.path(), Some(&path));
        }
        assert_eq!(observations.opens.load(Ordering::SeqCst), 0);
    }
}

/// Async dispatch has the same no-I/O validation boundary.
#[cfg(feature = "async")]
#[test]
fn test_range_overflow_precedes_async_dispatch() {
    let observations = Arc::new(Observations::default());
    let fs = AsyncFileSystem::from_spi(provider(&observations, true)).expect("filesystem");
    let path = Path::parse("/payload").expect("path");
    let options = ReadOptions::default().with_offset(Some(u64::MAX)).with_length(Some(1));
    let error = poll_support::ready(fs.open_reader(&path, options.clone())).expect_err("overflow");
    assert_eq!(error.kind(), FsErrorKind::InvalidOptions);
    let error = poll_support::ready(fs.read_prefix(&path, options, 0)).expect_err("overflow");
    assert_eq!(error.kind(), FsErrorKind::InvalidOptions);
    assert_eq!(observations.opens.load(Ordering::SeqCst), 0);
}

/// Empty windows still open once, including at the maximum representable
/// offset.
#[test]
fn test_empty_range_and_eof_preserve_open() {
    for (offset, length, expected) in [(u64::MAX, 0, &b""[..]), (16, 5, &b""[..]), (14, 8, &b"op"[..])] {
        let observations = Arc::new(Observations::default());
        let fs = FileSystem::from_spi(provider(&observations, true)).expect("filesystem");
        let options = ReadOptions::default()
            .with_offset(Some(offset))
            .with_length(Some(length));
        let bytes = fs
            .read_all(&Path::parse("/payload").expect("path"), options, 16)
            .expect("range");
        assert_eq!(bytes, expected);
        assert_eq!(observations.opens.load(Ordering::SeqCst), 1);
        assert_eq!(observations.stats.load(Ordering::SeqCst), 0);
    }
}
