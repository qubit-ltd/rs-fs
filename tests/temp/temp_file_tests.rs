// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use qubit_fs::{
    AchievedAtomicity, AtomicityRequirement, FileKind, FileLocation, FileMetadata, FileResource,
    FileSystem, FileSystemCapabilities, FileSystemId, FileSystemInfo, FileSystemLimit,
    FileSystemLimits, FileSystemProperties, FsError, FsErrorKind, FsOperation, FsPath, FsUri,
    PathSemantics, PersistFailure, PersistFailureState, PersistOptions, PersistOutcome,
    PublicationMethod, ReadOptions, TempFile, TempResourceSession, TempResourceState, WriteOptions,
};

use crate::common::MockFs;

#[derive(Clone, Copy, Debug)]
enum PersistBehavior {
    FailNotPublishedOnce,
    PublishedSourceRetained,
    Indeterminate,
    Succeed,
}

#[derive(Debug)]
struct TestTempSession {
    behavior: PersistBehavior,
    cleanup_calls: Arc<Mutex<usize>>,
}

impl TempResourceSession for TestTempSession {
    fn cleanup(&mut self) -> qubit_fs::FsResult<()> {
        *self
            .cleanup_calls
            .lock()
            .expect("cleanup lock should succeed") += 1;
        Ok(())
    }

    fn keep(&mut self) -> qubit_fs::FsResult<()> {
        Ok(())
    }

    fn persist(
        &mut self,
        target: &FsPath,
        _options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure> {
        match self.behavior {
            PersistBehavior::FailNotPublishedOnce => {
                self.behavior = PersistBehavior::Succeed;
                Err(PersistFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::PersistTemp,
                        "target was not published",
                    ),
                    PersistFailureState::NotPublished,
                ))
            }
            PersistBehavior::PublishedSourceRetained => Err(PersistFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CleanupTemp,
                    "target published but source cleanup failed",
                ),
                PersistFailureState::PublishedSourceRetained,
            )),
            PersistBehavior::Indeterminate => Err(PersistFailure::new(
                FsError::new(
                    FsErrorKind::Indeterminate,
                    FsOperation::PersistTemp,
                    "publication state is unknown",
                ),
                PersistFailureState::Indeterminate,
            )),
            PersistBehavior::Succeed => Ok(PersistOutcome::new(
                target.clone(),
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            )),
        }
    }
}

fn temp_file(behavior: PersistBehavior, cleanup_calls: Arc<Mutex<usize>>) -> TempFile {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    TempFile::new(
        FileResource::new(
            fs,
            FsPath::parse_normalized("/tmp/source").expect("path should parse"),
        ),
        TestTempSession {
            behavior,
            cleanup_calls,
        },
    )
}

#[test]
fn not_published_failure_retains_handle_and_allows_retry() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let mut temp = temp_file(PersistBehavior::FailNotPublishedOnce, cleanup_calls.clone());
    let target = FsPath::parse_normalized("/final").expect("path should parse");

    let failure = temp
        .persist(&target, PersistOptions::default())
        .expect_err("first persist should fail definitely");
    assert_eq!(PersistFailureState::NotPublished, failure.state());
    assert_eq!(TempResourceState::Owned, temp.state());
    assert_eq!("/tmp/source", temp.path().as_str());

    assert!(temp.persist(&target, PersistOptions::default()).is_ok());
    assert_eq!(TempResourceState::Persisted, temp.state());
    drop(temp);
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn published_source_retained_is_reported_as_partial_success() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let mut temp = temp_file(
        PersistBehavior::PublishedSourceRetained,
        cleanup_calls.clone(),
    );
    let target = FsPath::parse_normalized("/final").expect("path should parse");

    let failure = temp
        .persist(&target, PersistOptions::default())
        .expect_err("source cleanup should fail after publication");
    assert_eq!(
        PersistFailureState::PublishedSourceRetained,
        failure.state(),
    );
    assert_eq!(TempResourceState::CleanupRequired, temp.state());
    temp.cleanup().expect("source cleanup should be retryable");
    assert_eq!(TempResourceState::Cleaned, temp.state());
    assert_eq!(1, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_persist_is_not_automatically_retried_or_cleaned() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let mut temp = temp_file(PersistBehavior::Indeterminate, cleanup_calls.clone());
    let target = FsPath::parse_normalized("/final").expect("path should parse");

    let failure = temp
        .persist(&target, PersistOptions::default())
        .expect_err("persist should become indeterminate");
    assert_eq!(PersistFailureState::Indeterminate, failure.state());
    assert_eq!(TempResourceState::Indeterminate, temp.state());
    drop(temp);
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

#[derive(Debug)]
struct LifecycleSession {
    cleanup_results: VecDeque<Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    cleanup_calls: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
}

impl TempResourceSession for LifecycleSession {
    fn cleanup(&mut self) -> qubit_fs::FsResult<()> {
        *self
            .cleanup_calls
            .lock()
            .expect("cleanup lock should succeed") += 1;
        if let Some(Some(kind)) = self.cleanup_results.pop_front() {
            Err(FsError::new(
                kind,
                FsOperation::CleanupTemp,
                "cleanup failed",
            ))
        } else {
            Ok(())
        }
    }

    fn keep(&mut self) -> qubit_fs::FsResult<()> {
        if let Some(kind) = self.keep_error {
            Err(FsError::new(kind, FsOperation::CleanupTemp, "keep failed"))
        } else {
            Ok(())
        }
    }

    fn persist(
        &mut self,
        target: &FsPath,
        _options: PersistOptions,
    ) -> Result<PersistOutcome, PersistFailure> {
        *self
            .persist_calls
            .lock()
            .expect("persist lock should succeed") += 1;
        Ok(PersistOutcome::new(
            target.clone(),
            AchievedAtomicity::Atomic,
            PublicationMethod::AtomicRename,
        ))
    }
}

fn lifecycle_temp_file(
    fs: Arc<dyn FileSystem>,
    cleanup_results: impl IntoIterator<Item = Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    cleanup_calls: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
) -> TempFile {
    let location = FileLocation::new(
        fs.info().id().clone(),
        FsPath::parse("/tmp/source").unwrap(),
    )
    .with_uri(FsUri::parse("mock:///tmp/source").unwrap());
    TempFile::new(
        FileResource::from_location(fs, location),
        LifecycleSession {
            cleanup_results: cleanup_results.into_iter().collect(),
            keep_error,
            cleanup_calls,
            persist_calls,
        },
    )
}

#[test]
fn temp_file_accessors_streams_and_keep_preserve_resource_identity() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let mut temp = lifecycle_temp_file(fs, [], None, cleanup_calls.clone(), persist_calls);

    assert_eq!("/tmp/source", temp.path().as_str());
    assert_eq!(temp.path(), temp.resource().path());
    assert!(format!("{temp:?}").contains("TempFile"));
    let reader = temp.open_reader(ReadOptions::default()).unwrap();
    assert_eq!(temp.resource().location(), reader.info().location());
    let mut writer = temp.open_writer(WriteOptions::default()).unwrap();
    assert_eq!(temp.resource().location(), writer.info().location());
    writer.abort().unwrap();

    assert_eq!("/tmp/source", temp.keep().unwrap().as_str());
    assert_eq!(TempResourceState::Kept, temp.state());
    let keep_error = temp.keep().unwrap_err();
    assert_eq!(FsErrorKind::InvalidState, keep_error.kind());
    assert_eq!(FsOperation::KeepTemp, keep_error.operation());
    let cleanup_error = temp.cleanup().unwrap_err();
    assert_eq!(FsErrorKind::InvalidState, cleanup_error.kind());
    assert_eq!(FsOperation::CleanupTemp, cleanup_error.operation());
    let persist_failure = temp
        .persist(&FsPath::parse("/final").unwrap(), PersistOptions::default())
        .unwrap_err();
    assert_eq!(
        FsOperation::PersistTemp,
        persist_failure.error().operation()
    );
    assert_eq!(PersistFailureState::NotPublished, persist_failure.state(),);
    drop(temp);
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn temp_file_cleanup_failures_define_retry_and_indeterminate_states() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let mut retryable = lifecycle_temp_file(
        fs.clone(),
        [Some(FsErrorKind::Io), None],
        None,
        cleanup_calls.clone(),
        persist_calls.clone(),
    );

    assert!(retryable.cleanup().is_err());
    assert_eq!(TempResourceState::CleanupRequired, retryable.state());
    retryable.cleanup().expect("cleanup retry should succeed");
    assert_eq!(TempResourceState::Cleaned, retryable.state());
    assert!(retryable.cleanup().is_err());

    let mut indeterminate = lifecycle_temp_file(
        fs,
        [Some(FsErrorKind::Indeterminate)],
        None,
        cleanup_calls.clone(),
        persist_calls,
    );
    assert!(indeterminate.cleanup().is_err());
    assert_eq!(TempResourceState::Indeterminate, indeterminate.state());
    drop(indeterminate);
    assert_eq!(3, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn temp_file_keep_failure_and_drop_cleanup_failure_retain_responsibility() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    {
        let mut temp = lifecycle_temp_file(
            fs,
            [Some(FsErrorKind::Io)],
            Some(FsErrorKind::Io),
            cleanup_calls.clone(),
            persist_calls,
        );
        assert!(temp.keep().is_err());
        assert_eq!(TempResourceState::Owned, temp.state());
    }
    assert_eq!(1, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_temp_file_keep_disables_automatic_cleanup() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    {
        let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
        let mut temp = lifecycle_temp_file(
            fs,
            [],
            Some(FsErrorKind::Indeterminate),
            cleanup_calls.clone(),
            Arc::new(Mutex::new(0)),
        );

        assert_eq!(FsErrorKind::Indeterminate, temp.keep().unwrap_err().kind(),);
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

struct NoAtomicFileSystem {
    info: FileSystemInfo,
}

impl FileSystemProperties for NoAtomicFileSystem {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits = qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for NoAtomicFileSystem {
    fn stat(&self, _path: &FsPath) -> qubit_fs::FsResult<FileMetadata> {
        Ok(FileMetadata::new(FileKind::File))
    }
}

#[test]
fn temp_file_required_atomicity_fails_before_provider_persist() {
    let fs: Arc<dyn FileSystem> = Arc::new(NoAtomicFileSystem {
        info: FileSystemInfo::new(
            FileSystemId::new("no-atomic").unwrap(),
            "mock",
            PathSemantics::Hierarchical,
        ),
    });
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let mut temp = lifecycle_temp_file(fs, [], None, cleanup_calls, persist_calls.clone());
    let target = FsPath::parse("/final").unwrap();
    let options = PersistOptions {
        atomicity: AtomicityRequirement::Required,
        ..PersistOptions::default()
    };

    let failure = temp.persist(&target, options).unwrap_err();
    assert_eq!(PersistFailureState::NotPublished, failure.state());
    assert_eq!(Some(temp.path()), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
    assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
}

#[test]
fn temp_file_persist_preflights_target_path_limits() {
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(4));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default().with_limits(limits));
    let persist_calls = Arc::new(Mutex::new(0));
    let mut temp =
        lifecycle_temp_file(fs, [], None, Arc::new(Mutex::new(0)), persist_calls.clone());
    let target = FsPath::parse("/final").unwrap();

    let failure = temp
        .persist(&target, PersistOptions::default())
        .unwrap_err();

    assert_eq!(PersistFailureState::NotPublished, failure.state());
    assert_eq!(FsErrorKind::ResourceLimitExceeded, failure.error().kind());
    assert_eq!(Some(temp.path()), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
    assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
}
