// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::future::poll_fn;
#[cfg(feature = "async")]
use std::mem::replace;
#[cfg(feature = "async")]
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;
#[cfg(feature = "async")]
use std::task::Waker;

use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::ResourceVersion;
use qubit_fs::read::PrefixReadTermination;
use qubit_fs::read::ReadAllOptions;
use qubit_fs::read::ReadConsistencyPolicy;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncManagedReadPort;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncRandomAccessReadSession;
use qubit_fs::spi::ManagedReadPort;
use qubit_fs::spi::RandomAccessReadSession;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;

struct Bytes(Arc<Vec<u8>>);

impl RandomAccessReadSession for Bytes {
    fn read_at(&mut self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let Ok(offset) = usize::try_from(offset) else {
            return Ok(0);
        };
        let Some(source) = self.0.get(offset..) else {
            return Ok(0);
        };
        let count = source.len().min(output.len());
        output[..count].copy_from_slice(&source[..count]);
        Ok(count)
    }
}

fn read_port(data: &[u8], max_segment: usize, max_total: u64) -> ManagedReadPort {
    let info = OpenedFileInfo::new(
        FileSystemId::new("managed-read-test").unwrap(),
        Path::parse("/object").unwrap(),
    )
    .with_metadata(
        FileMetadata::new(FileKind::File)
            .with_len(Some(data.len() as u64))
            .with_etag(Some(ResourceVersion::new("v1"))),
    );
    ManagedReadPort::new(
        info,
        data.len() as u64,
        max_segment,
        max_total,
        Box::new(Bytes(Arc::new(data.to_vec()))),
    )
}

#[test]
fn test_read_all_reserves_one_sentinel_byte_without_overflow() {
    assert_eq!(
        ReadAllOptions::default().sentinel_capacity().unwrap(),
        16 * 1024 * 1024 + 1
    );
    assert_eq!(
        ReadAllOptions::new(0, Default::default())
            .sentinel_capacity()
            .unwrap(),
        1
    );
    assert!(
        ReadAllOptions::new(usize::MAX, Default::default())
            .sentinel_capacity()
            .is_err()
    );
}

#[test]
fn test_observe_changes_is_not_a_snapshot_claim() {
    let outcome = read_port(b"data", 2, 4)
        .read_all(ReadAllOptions::new(
            4,
            ReadConsistencyPolicy::ObserveChanges,
        ))
        .unwrap();
    assert_eq!(outcome.value, b"data");
    assert_eq!(outcome.changed, None);
    assert_eq!(outcome.version.unwrap().as_str(), "v1");
}

#[test]
fn test_strict_text_decoding_preserves_utf8_failure_as_source() {
    let error = ReadAllOptions::decode_text(vec![0xff]).unwrap_err();
    assert_eq!(error.kind(), FsErrorKind::DataCorruption);
    assert!(Error::source(&error).is_some());
}

#[test]
fn test_managed_read_port_stays_on_its_opened_object_and_limits_segments() {
    let original = Arc::new(b"old-object".to_vec());
    let path_binding = Arc::new(Mutex::new(Arc::clone(&original)));
    let info = OpenedFileInfo::new(
        FileSystemId::new("managed-read-test").unwrap(),
        Path::parse("/object").unwrap(),
    )
    .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(original.len() as u64)));
    let port = ManagedReadPort::new(
        info,
        original.len() as u64,
        3,
        16,
        Box::new(Bytes(original)),
    );
    // Replace the path's current binding after open. The port retains the
    // original object reference captured by its session.
    *path_binding.lock().unwrap() = Arc::new(b"new-object".to_vec());
    assert_eq!(path_binding.lock().unwrap().as_slice(), b"new-object");
    let mut bytes = [0; 8];
    let count = port.read_at(0, &mut bytes).unwrap();
    assert_eq!(&bytes[..count], b"old");
    assert_eq!(port.info().unwrap().path().as_str(), "/object");
}

#[test]
fn test_managed_read_port_counts_repeated_ranges_and_rejects_offset_overflow() {
    let port = read_port(b"abcdef", 4, 4);
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ManagedReadPort>();
    let cloned_port = port.clone();
    let mut bytes = [0; 4];
    assert_eq!(cloned_port.read_at(1, &mut bytes).unwrap(), 4);
    assert_eq!(&bytes, b"bcde");
    assert_eq!(
        port.read_at(1, &mut bytes).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
    assert_eq!(
        port.read_at(u64::MAX, &mut bytes).unwrap_err().kind(),
        FsErrorKind::InvalidOptions
    );
}

#[test]
fn test_managed_read_all_accepts_exact_limit_and_rejects_limit_plus_one() {
    let exact = read_port(b"four", 2, 5)
        .read_all(ReadAllOptions::new(4, Default::default()))
        .unwrap();
    assert_eq!(exact.value, b"four");
    assert_eq!(exact.changed, None);

    let oversized = read_port(b"fives", 2, 6)
        .read_all(ReadAllOptions::new(4, Default::default()))
        .unwrap_err();
    assert_eq!(oversized.kind(), FsErrorKind::ResourceLimitExceeded);

    assert_eq!(
        read_port(b"", 2, 0)
            .read_all(ReadAllOptions::new(0, Default::default()))
            .unwrap()
            .value,
        b""
    );
    assert_eq!(
        read_port(b"x", 2, 2)
            .read_all(ReadAllOptions::new(usize::MAX, Default::default()))
            .unwrap_err()
            .kind(),
        FsErrorKind::ResourceLimitExceeded
    );
    assert_eq!(
        read_port(b"x", 2, 2)
            .read_all(ReadAllOptions::new(
                1,
                ReadConsistencyPolicy::RequireVersion,
            ))
            .unwrap_err()
            .kind(),
        FsErrorKind::RequirementNotMet
    );
}

#[test]
fn test_managed_prefix_marks_limit_without_claiming_eof_and_text_is_strict() {
    let prefix = read_port(b"abcdef", 2, 6).read_prefix(3).unwrap();
    assert_eq!(prefix.bytes(), b"abc");
    assert_eq!(prefix.termination(), PrefixReadTermination::LimitReached);

    assert_eq!(
        read_port("héllo".as_bytes(), 3, 6)
            .read_text(ReadAllOptions::new(6, Default::default()))
            .unwrap(),
        "héllo"
    );
    assert_eq!(
        read_port(&[0xff], 1, 2)
            .read_text(ReadAllOptions::new(1, Default::default()))
            .unwrap_err()
            .kind(),
        FsErrorKind::DataCorruption
    );
}

#[test]
fn test_read_allocations_are_capped_by_object_and_remaining_session_budget() {
    let complete = read_port(b"tiny", 4, 4)
        .read_all(ReadAllOptions::new(usize::MAX - 1, Default::default()))
        .unwrap();
    assert_eq!(complete.value, b"tiny");

    let prefix = read_port(b"tiny", 4, 4).read_prefix(usize::MAX).unwrap();
    assert_eq!(prefix.bytes(), b"tiny");
    assert_eq!(prefix.termination(), PrefixReadTermination::StreamEnded);
}

#[test]
fn test_zero_segment_is_a_typed_error_for_non_empty_sync_reads() {
    let port = read_port(b"data", 0, 4);
    let mut output = [0; 1];
    assert_eq!(
        port.read_at(0, &mut output).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
    assert_eq!(
        port.read_all(ReadAllOptions::new(4, Default::default()))
            .unwrap_err()
            .kind(),
        FsErrorKind::ResourceLimitExceeded
    );
    assert_eq!(
        port.read_prefix(4).unwrap_err().kind(),
        FsErrorKind::ResourceLimitExceeded
    );
}

#[cfg(feature = "async")]
struct PendingOnce(Arc<Vec<u8>>, bool);

#[cfg(feature = "async")]
impl AsyncRandomAccessReadSession for PendingOnce {
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        output: &'a mut [u8],
    ) -> SpiFuture<'a, FsResult<usize>> {
        let session = self.get_mut();
        let bytes = Arc::clone(&session.0);
        let mut pending = replace(&mut session.1, false);
        Box::pin(poll_fn(move |_| {
            if pending {
                pending = false;
                output[0] = bytes[usize::try_from(offset).unwrap()];
                return Poll::Pending;
            }
            let start = usize::try_from(offset).unwrap();
            let count = output.len().min(bytes.len().saturating_sub(start));
            output[..count].copy_from_slice(&bytes[start..start + count]);
            Poll::Ready(Ok(count))
        }))
    }
}

#[cfg(feature = "async")]
#[test]
fn test_dropping_pending_async_read_allows_complete_retry_at_same_offset() {
    let data = Arc::new(b"original".to_vec());
    let info = OpenedFileInfo::new(
        FileSystemId::new("managed-read-test").unwrap(),
        Path::parse("/async-object").unwrap(),
    );
    let mut port = Box::pin(AsyncManagedReadPort::new(
        info,
        data.len() as u64,
        8,
        8,
        Box::pin(PendingOnce(data, true)),
    ));
    let mut partial = [0; 3];
    let mut pending = Box::pin(port.as_mut().read_at_async(0, &mut partial));
    let mut context = Context::from_waker(Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    drop(pending);
    assert_eq!(partial[0], b'o');

    let mut retry = [0; 3];
    let mut future = Box::pin(port.as_mut().read_at_async(0, &mut retry));
    assert!(matches!(
        future.as_mut().poll(&mut context),
        Poll::Ready(Ok(3))
    ));
    drop(future);
    assert_eq!(&retry, b"ori");
}

#[cfg(feature = "async")]
#[test]
fn test_zero_segment_is_a_typed_error_for_non_empty_async_reads() {
    let data = Arc::new(b"data".to_vec());
    let info = OpenedFileInfo::new(
        FileSystemId::new("managed-read-test").unwrap(),
        Path::parse("/async-object").unwrap(),
    );
    let mut port = Box::pin(AsyncManagedReadPort::new(
        info,
        data.len() as u64,
        0,
        4,
        Box::pin(PendingOnce(data, false)),
    ));
    let mut output = [0; 1];
    let mut future = Box::pin(port.as_mut().read_at_async(0, &mut output));
    let mut context = Context::from_waker(Waker::noop());
    let Poll::Ready(Err(error)) = future.as_mut().poll(&mut context) else {
        panic!("zero segment must fail before provider polling")
    };
    assert_eq!(error.kind(), FsErrorKind::ResourceLimitExceeded);
}
