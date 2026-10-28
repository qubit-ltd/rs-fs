// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Whole-file failure facts remain frozen across recovery.

mod handle_support;
use std::error::Error;

use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::PublicationMethod;
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
use qubit_fs::write::WriteDisposition;
use qubit_fs::write::WriteFailureState;
use qubit_fs::write::WriteOptions;
use qubit_fs::write::WriterRecovery;
use qubit_io::Output;

type SharedBytes = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;
type SharedCount = std::sync::Arc<std::sync::Mutex<usize>>;

#[derive(Clone)]
enum WriteStep {
    Accept(usize),
    Interrupted,
    TypedInterrupted(FsEffectState),
}

struct InterruptedProvider {
    steps: Vec<WriteStep>,
    accepted: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    calls: std::sync::Arc<std::sync::Mutex<usize>>,
    commits: std::sync::Arc<std::sync::Mutex<usize>>,
}

struct InterruptedWriter {
    steps: std::vec::IntoIter<WriteStep>,
    accepted: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    calls: std::sync::Arc<std::sync::Mutex<usize>>,
    commits: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl Output for InterruptedWriter {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, _count: usize) -> std::io::Result<usize> {
        *self.calls.lock().expect("calls lock") += 1;
        match self.steps.next().expect("script step") {
            WriteStep::Accept(accepted) => {
                self.accepted
                    .lock()
                    .expect("accepted lock")
                    .extend_from_slice(&input[index..index + accepted]);
                Ok(accepted)
            }
            WriteStep::Interrupted => Err(std::io::Error::from(std::io::ErrorKind::Interrupted)),
            WriteStep::TypedInterrupted(effect) => Err(FsError::with_source(
                FsErrorKind::Interrupted,
                FsOperation::Write,
                "provider interrupted",
                std::io::Error::other("provider source"),
            )
            .with_effect_state(effect)
            .into_io_error()),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl FileWriterSpi for InterruptedWriter {
    fn commit(&mut self) -> Result<WriteOutcome, qubit_fs::spi::SpiWriteFailure> {
        *self.commits.lock().expect("commits lock") += 1;
        Ok(WriteOutcome::new(
            qubit_fs::metadata::AchievedAtomicity::NonAtomic,
            qubit_fs::metadata::PublicationMethod::Direct,
        )
        .with_bytes_written(self.accepted.lock().expect("accepted lock").len() as u64))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}

impl FileSystemSpi for InterruptedProvider {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("interrupted").expect("id"),
                "interrupted",
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
        Err(FsError::new(FsErrorKind::NotFound, FsOperation::Stat, "missing"))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(self.properties().info().id().clone(), request.path().clone()),
            Box::new(InterruptedWriter {
                steps: self.steps.clone().into_iter(),
                accepted: self.accepted.clone(),
                calls: self.calls.clone(),
                commits: self.commits.clone(),
            }),
        ))
    }
}

fn interrupted_filesystem(steps: Vec<WriteStep>) -> (FileSystem, SharedBytes, SharedCount, SharedCount) {
    let accepted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let calls = std::sync::Arc::new(std::sync::Mutex::new(0));
    let commits = std::sync::Arc::new(std::sync::Mutex::new(0));
    let fs = FileSystem::from_spi(InterruptedProvider {
        steps,
        accepted: accepted.clone(),
        calls: calls.clone(),
        commits: commits.clone(),
    })
    .expect("facade");
    (fs, accepted, calls, commits)
}

#[test]
fn test_write_all_interrupted_before_acceptance_retries() {
    let (fs, accepted, calls, commits) = interrupted_filesystem(vec![WriteStep::Interrupted, WriteStep::Accept(4)]);
    let outcome = fs
        .write_all(&Path::parse("/target").expect("path"), b"data", WriteOptions::default())
        .expect("interrupted write retries");
    assert_eq!(Some(4), outcome.bytes_written());
    assert_eq!(b"data", accepted.lock().expect("accepted").as_slice());
    assert_eq!(2, *calls.lock().expect("calls"));
    assert_eq!(1, *commits.lock().expect("commits"));
}

#[test]
fn test_write_all_interrupted_with_unchanged_effect_retries() {
    let (fs, accepted, calls, commits) = interrupted_filesystem(vec![
        WriteStep::TypedInterrupted(FsEffectState::Unchanged),
        WriteStep::Accept(4),
    ]);
    let outcome = fs
        .write_all(&Path::parse("/target").expect("path"), b"data", WriteOptions::default())
        .expect("unchanged interruption retries");
    assert_eq!(Some(4), outcome.bytes_written());
    assert_eq!(b"data", accepted.lock().expect("accepted").as_slice());
    assert_eq!(2, *calls.lock().expect("calls"));
    assert_eq!(1, *commits.lock().expect("commits"));
}

#[test]
fn test_write_all_interrupted_after_short_write_retries_remaining_payload() {
    let (fs, accepted, calls, commits) =
        interrupted_filesystem(vec![WriteStep::Accept(2), WriteStep::Interrupted, WriteStep::Accept(2)]);
    let outcome = fs
        .write_all(&Path::parse("/target").expect("path"), b"data", WriteOptions::default())
        .expect("interrupted write retries");
    assert_eq!(Some(4), outcome.bytes_written());
    assert_eq!(b"data", accepted.lock().expect("accepted").as_slice());
    assert_eq!(3, *calls.lock().expect("calls"));
    assert_eq!(1, *commits.lock().expect("commits"));
}

#[test]
fn test_write_all_interrupted_with_partial_effect_is_not_retried() {
    let (fs, accepted, calls, commits) =
        interrupted_filesystem(vec![WriteStep::TypedInterrupted(FsEffectState::PartiallyApplied)]);
    let failure = fs
        .write_all(&Path::parse("/target").expect("path"), b"data", WriteOptions::default())
        .expect_err("uncertain interruption fails");
    assert_eq!(WriteFailureState::Indeterminate, failure.state());
    assert_eq!(0, failure.written_bytes());
    assert_eq!(1, *calls.lock().expect("calls"));
    assert_eq!(0, *commits.lock().expect("commits"));
    assert!(accepted.lock().expect("accepted").is_empty());
    assert_eq!(Some(FsEffectState::PartiallyApplied), failure.error().effect_state());
    assert_eq!(FsErrorKind::Interrupted, failure.error().kind());
    assert!(std::error::Error::source(failure.error()).is_some());
}

struct ZeroProgressAppendProbe {
    target: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    staging: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    direct_append: bool,
    writes: std::sync::Arc<std::sync::Mutex<usize>>,
    commits: std::sync::Arc<std::sync::Mutex<usize>>,
    aborts: std::sync::Arc<std::sync::Mutex<usize>>,
}

struct ZeroProgressAppendWriter {
    target: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    staging: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    direct_append: bool,
    writes: std::sync::Arc<std::sync::Mutex<usize>>,
    commits: std::sync::Arc<std::sync::Mutex<usize>>,
    aborts: std::sync::Arc<std::sync::Mutex<usize>>,
}

impl Output for ZeroProgressAppendWriter {
    type Item = u8;

    unsafe fn write_unchecked(&mut self, input: &[u8], index: usize, count: usize) -> std::io::Result<usize> {
        let mut writes = self.writes.lock().expect("writes lock");
        *writes += 1;
        if *writes > 1 {
            return Ok(0);
        }
        let accepted = count.min(2);
        if self.direct_append {
            self.target
                .lock()
                .expect("target lock")
                .extend_from_slice(&input[index..index + accepted]);
        } else {
            self.staging
                .lock()
                .expect("staging lock")
                .extend_from_slice(&input[index..index + accepted]);
        }
        Ok(accepted)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl FileWriterSpi for ZeroProgressAppendWriter {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        *self.commits.lock().expect("commits lock") += 1;
        if !self.direct_append {
            self.target
                .lock()
                .expect("target lock")
                .extend_from_slice(self.staging.lock().expect("staging lock").as_slice());
        }
        Ok(
            WriteOutcome::new(AchievedAtomicity::NonAtomic, PublicationMethod::Direct)
                .with_bytes_written(self.target.lock().expect("target lock").len() as u64),
        )
    }

    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        *self.aborts.lock().expect("aborts lock") += 1;
        if !self.direct_append {
            self.staging.lock().expect("staging lock").clear();
        }
        Ok(
            if self.direct_append && !self.target.lock().expect("target lock").is_empty() {
                WriteAbortOutcome::Published
            } else {
                WriteAbortOutcome::NotPublished
            },
        )
    }
}

impl FileSystemSpi for ZeroProgressAppendProbe {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("zero-progress-append").expect("id"),
                "zero-progress-append",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenWriter),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Write)
                .with_guaranteed(FileSystemCapability::Append),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("properties")
    }

    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(FsError::new(FsErrorKind::NotFound, FsOperation::Stat, "missing"))
    }

    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        assert_eq!(
            if self.direct_append {
                WriteDisposition::Append
            } else {
                WriteDisposition::CreateNew
            },
            request.options().options().disposition()
        );
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(self.properties().info().id().clone(), request.path().clone()),
            Box::new(ZeroProgressAppendWriter {
                target: self.target.clone(),
                staging: self.staging.clone(),
                direct_append: self.direct_append,
                writes: self.writes.clone(),
                commits: self.commits.clone(),
                aborts: self.aborts.clone(),
            }),
        ))
    }
}

fn zero_progress_filesystem(direct_append: bool) -> (FileSystem, SharedBytes, SharedCount, SharedCount, SharedCount) {
    let target = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let staging = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(0));
    let commits = std::sync::Arc::new(std::sync::Mutex::new(0));
    let aborts = std::sync::Arc::new(std::sync::Mutex::new(0));
    let filesystem = FileSystem::from_spi(ZeroProgressAppendProbe {
        target: target.clone(),
        staging,
        direct_append,
        writes: writes.clone(),
        commits: commits.clone(),
        aborts: aborts.clone(),
    })
    .expect("facade");
    (filesystem, target, writes, commits, aborts)
}

#[test]
fn test_write_all_zero_progress_after_append_prefix_preserves_publication_facts() {
    let (filesystem, target, writes, commits, aborts) = zero_progress_filesystem(true);
    let mut failure = filesystem
        .write_all(
            &Path::parse("/target").expect("path"),
            b"abcd",
            WriteOptions::default().with_disposition(WriteDisposition::Append),
        )
        .expect_err("nonempty zero progress is WriteZero");

    assert_eq!(b"ab", target.lock().expect("target").as_slice());
    assert_eq!(2, failure.written_bytes());
    assert_eq!(WriteFailureState::Indeterminate, failure.state());
    assert_eq!(2, *writes.lock().expect("writes"));
    assert_eq!(0, *commits.lock().expect("commits"));
    assert_eq!(0, *aborts.lock().expect("aborts"));
    assert_eq!(
        std::io::ErrorKind::WriteZero,
        failure
            .error()
            .source()
            .and_then(|source| source.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind)
            .unwrap(),
    );
    let Some(WriterRecovery::Opened(writer)) = failure.recovery_mut() else {
        panic!("append writer retained for recovery");
    };
    assert_eq!(
        WriteAbortOutcome::Published,
        writer.abort().expect("abort confirms visible append")
    );
    assert_eq!(1, *aborts.lock().expect("aborts"));
    assert_eq!(b"ab", target.lock().expect("target").as_slice());
    assert_eq!(WriteFailureState::Indeterminate, failure.state());
    assert_eq!(2, failure.written_bytes());
}

#[test]
fn test_write_all_zero_progress_empty_payload_is_successful() {
    let (filesystem, target, writes, commits, aborts) = zero_progress_filesystem(true);
    let outcome = filesystem
        .write_all(
            &Path::parse("/empty").expect("path"),
            b"",
            WriteOptions::default().with_disposition(WriteDisposition::Append),
        )
        .expect("empty payload succeeds");
    assert_eq!(Some(0), outcome.bytes_written());
    assert!(target.lock().expect("target").is_empty());
    assert_eq!(0, *writes.lock().expect("writes"));
    assert_eq!(1, *commits.lock().expect("commits"));
    assert_eq!(0, *aborts.lock().expect("aborts"));
}

#[test]
fn test_write_all_zero_progress_after_staged_prefix_remains_unpublished() {
    let (filesystem, target, writes, commits, aborts) = zero_progress_filesystem(false);
    let mut failure = filesystem
        .write_all(
            &Path::parse("/target").expect("path"),
            b"abcd",
            WriteOptions::default().with_disposition(WriteDisposition::CreateNew),
        )
        .expect_err("nonempty zero progress is WriteZero");

    assert!(target.lock().expect("target").is_empty());
    assert_eq!(2, failure.written_bytes());
    assert_eq!(WriteFailureState::RetryableNotPublished, failure.state());
    assert_eq!(2, *writes.lock().expect("writes"));
    assert_eq!(0, *commits.lock().expect("commits"));
    assert_eq!(0, *aborts.lock().expect("aborts"));
    let Some(WriterRecovery::Opened(writer)) = failure.recovery_mut() else {
        panic!("staged writer retained for recovery");
    };
    assert_eq!(
        WriteAbortOutcome::NotPublished,
        writer.abort().expect("staging cleanup succeeds")
    );
    assert_eq!(1, *aborts.lock().expect("aborts"));
    assert!(target.lock().expect("target").is_empty());
    assert_eq!(WriteFailureState::RetryableNotPublished, failure.state());
    assert_eq!(2, failure.written_bytes());
}

/// Provider-confirmed commit states survive cleanup and ownership transfer.
#[test]
fn test_write_all_commit_failure_preserves_snapshot() {
    for state in [
        WriteFailureState::RetryableNotPublished,
        WriteFailureState::NotPublished,
        WriteFailureState::Published,
        WriteFailureState::Indeterminate,
    ] {
        let fs = handle_support::implementation_tests::writer_lifecycle_filesystem(Some(state), None);
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
            handle_support::implementation_tests::limited_write_filesystem(1),
            WriteFailureState::NotPublished,
        ),
        (
            handle_support::implementation_tests::provider_open_failure_filesystem(),
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
