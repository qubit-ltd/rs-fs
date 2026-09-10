// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Whole-file failure facts remain frozen across recovery.

mod handle_support;
use qubit_fs::Path;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterRecovery;

/// Provider-confirmed commit states survive cleanup and ownership transfer.
#[test]
fn test_write_all_commit_failure_preserves_snapshot() {
    for state in [
        WriteFailureState::RetryableNotPublished,
        WriteFailureState::NotPublished,
        WriteFailureState::Published,
        WriteFailureState::Indeterminate,
    ] {
        let fs = handle_support::writer_lifecycle_filesystem(Some(state), None);
        let mut failure = fs
            .write_all(
                &Path::parse("/target").expect("path"),
                b"bytes",
                WriteOptions::default(),
            )
            .expect_err("commit failure");
        assert_eq!(failure.state(), state);
        assert_eq!(failure.written_bytes(), 5);
        let Some(WriterRecovery::Opened(writer)) = failure.recovery_mut() else {
            panic!("validated writer retained")
        };
        let _outcome = writer.abort().expect("cleanup");
        assert_eq!(failure.state(), state);
        assert_eq!(failure.written_bytes(), 5);
        drop(failure.take_recovery().expect("owned recovery"));
        assert!(failure.recovery().is_none());
        let (_error, actual_state, bytes, recovery) = failure.into_parts();
        assert_eq!(actual_state, state);
        assert_eq!(bytes, 5);
        assert!(recovery.is_none());
    }
}

/// Preflight and provider-open failures freeze zero bytes without inventing a
/// session.
#[test]
fn test_write_all_snapshot_before_a_session_exists() {
    for (filesystem, expected) in [
        (
            handle_support::limited_write_filesystem(1),
            WriteFailureState::NotPublished,
        ),
        (
            handle_support::provider_open_failure_filesystem(),
            WriteFailureState::Indeterminate,
        ),
    ] {
        let mut failure = filesystem
            .write_all(
                &Path::parse("/target").expect("path"),
                b"bytes",
                WriteOptions::default(),
            )
            .expect_err("pre-write failure");
        assert_eq!(failure.state(), expected);
        assert_eq!(failure.written_bytes(), 0);
        assert!(failure.take_recovery().is_none());
        let (_, state, bytes, recovery) = failure.into_parts();
        assert_eq!(state, expected);
        assert_eq!(bytes, 0);
        assert!(recovery.is_none());
    }
}

/// Short writes contribute only their acknowledgements, even when later stages
/// fail.
#[test]
fn test_partial_write_and_flush_failures_preserve_confirmed_bytes() {
    use qubit_fs::FileSystem;
    use qubit_fs::FsResult;
    use qubit_fs::error::FsError;
    use qubit_fs::error::FsErrorKind;
    use qubit_fs::error::FsOperation;
    use qubit_fs::metadata::FileSystemCapabilities;
    use qubit_fs::metadata::FileSystemCapability;
    use qubit_fs::metadata::FileSystemId;
    use qubit_fs::metadata::FileSystemInfo;
    use qubit_fs::metadata::FileSystemLimits;
    use qubit_fs::metadata::OpenedFileInfo;
    use qubit_fs::metadata::SymlinkPolicy;
    use qubit_fs::metadata::WriteOutcome;
    use qubit_fs::path::PathConstraints;
    use qubit_fs::path::PathSemantics;
    use qubit_fs::spi::FileSystemSpi;
    use qubit_fs::spi::FileWriterSpi;
    use qubit_fs::spi::OpenWriterRequest;
    use qubit_fs::spi::OpenedWriter;
    use qubit_fs::spi::ProviderOperation;
    use qubit_fs::spi::ProviderOperations;
    use qubit_fs::spi::ProviderProperties;
    use qubit_fs::spi::SpiWriteFailure;
    use qubit_fs::spi::StatRequest;
    use qubit_fs::spi::StatResponse;
    use qubit_fs::write::WriteAbortOutcome;
    use qubit_io::Output;

    struct Provider {
        flush_failure: bool,
    }
    struct Writer {
        accepted: usize,
        flush_failure: bool,
    }
    impl Output for Writer {
        type Item = u8;
        unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> std::io::Result<usize> {
            if self.accepted == 5 {
                return Err(std::io::Error::other("write failed after two acknowledgements"));
            }
            let accepted = count.min(if self.accepted == 0 { 2 } else { 3 });
            self.accepted += accepted;
            Ok(accepted)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            assert!(self.flush_failure, "partial failure must stop before flush");
            Err(std::io::Error::other("flush failed"))
        }
    }
    impl FileWriterSpi for Writer {
        fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
            panic!("stream failure must prevent commit")
        }
        fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
            self.accepted = 0;
            Ok(WriteAbortOutcome::NotPublished)
        }
    }
    impl FileSystemSpi for Provider {
        fn properties(&self) -> ProviderProperties {
            ProviderProperties::new(
                FileSystemInfo::new(
                    FileSystemId::new("short-write").expect("id"),
                    "short-write",
                    PathSemantics::Hierarchical,
                ),
                ProviderOperations::new()
                    .with(ProviderOperation::Stat)
                    .with(ProviderOperation::OpenWriter),
                FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Write),
                FileSystemLimits::unknown(),
                PathConstraints::absolute(),
                SymlinkPolicy::Reject,
            )
            .expect("properties")
        }
        fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
            Err(FsError::new(
                FsErrorKind::NotFound,
                FsOperation::Stat,
                "no published file",
            ))
        }
        fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
            Ok(OpenedWriter::new(
                OpenedFileInfo::new(self.properties().info().id().clone(), request.path().clone()),
                Box::new(Writer {
                    accepted: 0,
                    flush_failure: self.flush_failure,
                }),
            ))
        }
    }
    for flush_failure in [false, true] {
        let filesystem = FileSystem::from_spi(Provider { flush_failure }).expect("facade");
        let payload: &[u8] = if flush_failure { b"12345" } else { b"1234567" };
        let mut failure = filesystem
            .write_all(&Path::parse("/target").expect("path"), payload, WriteOptions::default())
            .expect_err("stream failure");
        assert_eq!(failure.state(), WriteFailureState::Indeterminate);
        assert_eq!(failure.written_bytes(), 5);
        let Some(WriterRecovery::Opened(writer)) = failure.recovery_mut() else {
            panic!("writer retained")
        };
        assert_eq!(
            writer.abort().expect("explicit cleanup"),
            WriteAbortOutcome::NotPublished
        );
        assert_eq!(failure.state(), WriteFailureState::Indeterminate);
        assert_eq!(failure.written_bytes(), 5);
        let (_, state, count, recovery) = failure.into_parts();
        assert_eq!(state, WriteFailureState::Indeterminate);
        assert_eq!(count, 5);
        assert!(recovery.is_some());
    }
}
