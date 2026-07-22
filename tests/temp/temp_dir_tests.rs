// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::VecDeque;
use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    CreateDirOptions,
    FileKind,
    FileMetadata,
    FileResource,
    FileSystem,
    FileSystemCapabilities,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsName,
    FsPath,
    ListOptions,
    PathSemantics,
    PersistFailure,
    PersistFailureState,
    PersistOptions,
    PersistOutcome,
    PublicationMethod,
    RelativeFsPath,
    TempDir,
    TempResourceSession,
    TempResourceState,
};
use qubit_spi::ProviderId;

use crate::common::MockFs;

#[derive(Debug)]
struct SuccessfulSession;

impl TempResourceSession for SuccessfulSession {
    fn cleanup(&mut self) -> qubit_fs::FsResult<()> {
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
        Ok(PersistOutcome::new(
            target.clone(),
            AchievedAtomicity::Atomic,
            PublicationMethod::AtomicRename,
        ))
    }
}

fn temp_dir() -> TempDir {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    TempDir::new(
        FileResource::new(
            fs,
            FsPath::parse_normalized("/tmp/root").expect("path should parse"),
        ),
        SuccessfulSession,
    )
}

#[test]
fn child_and_descendant_use_safe_relative_types() {
    let temp = temp_dir();
    let child =
        temp.child(&FsName::parse("child.txt").expect("name should parse"));
    let descendant = temp.descendant(
        &RelativeFsPath::parse("nested/file.txt")
            .expect("relative path should parse"),
    );

    assert_eq!("/tmp/root/child.txt", child.path().as_str());
    assert_eq!("/tmp/root/nested/file.txt", descendant.path().as_str());
}

#[test]
fn absolute_and_escaping_children_are_rejected_before_temp_dir_access() {
    assert!(FsName::parse("/etc/passwd").is_err());
    assert!(RelativeFsPath::parse("/etc/passwd").is_err());
    assert!(RelativeFsPath::parse("../escape").is_err());
}

#[derive(Debug)]
struct DirectoryLifecycleSession {
    cleanup_results: VecDeque<Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    persist_failure: Option<PersistFailureState>,
    cleanup_calls: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
}

impl TempResourceSession for DirectoryLifecycleSession {
    fn cleanup(&mut self) -> qubit_fs::FsResult<()> {
        *self
            .cleanup_calls
            .lock()
            .expect("cleanup lock should succeed") += 1;
        if let Some(Some(kind)) = self.cleanup_results.pop_front() {
            Err(FsError::new(
                kind,
                qubit_fs::FsOperation::CleanupTemp,
                "cleanup failed",
            ))
        } else {
            Ok(())
        }
    }

    fn keep(&mut self) -> qubit_fs::FsResult<()> {
        if let Some(kind) = self.keep_error {
            Err(FsError::new(
                kind,
                qubit_fs::FsOperation::CleanupTemp,
                "keep failed",
            ))
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
        if let Some(state) = self.persist_failure {
            let kind = if state == PersistFailureState::Indeterminate {
                FsErrorKind::Indeterminate
            } else {
                FsErrorKind::Io
            };
            Err(PersistFailure::new(
                FsError::new(
                    kind,
                    qubit_fs::FsOperation::PersistTemp,
                    "persist failed",
                ),
                state,
            ))
        } else {
            Ok(PersistOutcome::new(
                target.clone(),
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            ))
        }
    }
}

fn lifecycle_temp_dir(
    fs: Arc<dyn FileSystem>,
    cleanup_results: impl IntoIterator<Item = Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    persist_failure: Option<PersistFailureState>,
    cleanup_calls: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
) -> TempDir {
    TempDir::new(
        FileResource::new(fs, FsPath::parse("/tmp/root").unwrap()),
        DirectoryLifecycleSession {
            cleanup_results: cleanup_results.into_iter().collect(),
            keep_error,
            persist_failure,
            cleanup_calls,
            persist_calls,
        },
    )
}

#[test]
fn temp_dir_accessors_listing_child_creation_and_keep_are_usable() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let mut temp = lifecycle_temp_dir(
        fs,
        [],
        None,
        None,
        cleanup_calls.clone(),
        persist_calls,
    );

    assert_eq!(temp.path(), temp.resource().path());
    assert!(format!("{temp:?}").contains("TempDir"));
    assert!(temp.list(ListOptions::default()).is_ok());
    let child = temp
        .create_child_dir(
            &FsName::parse("child-dir").unwrap(),
            CreateDirOptions::default(),
        )
        .unwrap();
    assert_eq!("/tmp/root/child-dir", child.path().as_str());

    assert_eq!("/tmp/root", temp.keep().unwrap().as_str());
    assert_eq!(TempResourceState::Kept, temp.state());
    assert!(temp.keep().is_err());
    assert!(temp.cleanup().is_err());
    assert!(
        temp.persist(
            &FsPath::parse("/final").unwrap(),
            PersistOptions::default()
        )
        .is_err(),
    );
    drop(temp);
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn temp_dir_cleanup_retry_indeterminate_and_drop_failure_are_explicit() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let mut retryable = lifecycle_temp_dir(
        fs.clone(),
        [Some(FsErrorKind::Io), None],
        None,
        None,
        cleanup_calls.clone(),
        persist_calls.clone(),
    );
    assert!(retryable.cleanup().is_err());
    assert_eq!(TempResourceState::CleanupRequired, retryable.state());
    retryable.cleanup().unwrap();
    assert_eq!(TempResourceState::Cleaned, retryable.state());

    let mut indeterminate = lifecycle_temp_dir(
        fs.clone(),
        [Some(FsErrorKind::Indeterminate)],
        None,
        None,
        cleanup_calls.clone(),
        persist_calls.clone(),
    );
    assert!(indeterminate.cleanup().is_err());
    assert_eq!(TempResourceState::Indeterminate, indeterminate.state());
    drop(indeterminate);

    {
        let _drop_failure = lifecycle_temp_dir(
            fs,
            [Some(FsErrorKind::Io)],
            None,
            None,
            cleanup_calls.clone(),
            persist_calls,
        );
    }
    assert_eq!(4, *cleanup_calls.lock().expect("lock should succeed"));
}

#[test]
fn temp_dir_keep_failure_and_all_persist_states_retain_recovery_contract() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let target = FsPath::parse("/final").unwrap();

    let mut keep_failure = lifecycle_temp_dir(
        fs.clone(),
        [],
        Some(FsErrorKind::Io),
        None,
        Arc::new(Mutex::new(0)),
        Arc::new(Mutex::new(0)),
    );
    assert!(keep_failure.keep().is_err());
    assert_eq!(TempResourceState::Owned, keep_failure.state());

    let cases = [
        (PersistFailureState::NotPublished, TempResourceState::Owned),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
        ),
        (
            PersistFailureState::Indeterminate,
            TempResourceState::Indeterminate,
        ),
    ];
    for (failure_state, resource_state) in cases {
        let mut temp = lifecycle_temp_dir(
            fs.clone(),
            [],
            None,
            Some(failure_state),
            Arc::new(Mutex::new(0)),
            Arc::new(Mutex::new(0)),
        );
        assert_eq!(
            failure_state,
            temp.persist(&target, PersistOptions::default())
                .unwrap_err()
                .state(),
        );
        assert_eq!(resource_state, temp.state());
    }

    let mut success = lifecycle_temp_dir(
        fs,
        [],
        None,
        None,
        Arc::new(Mutex::new(0)),
        Arc::new(Mutex::new(0)),
    );
    assert!(success.persist(&target, PersistOptions::default()).is_ok());
    assert_eq!(TempResourceState::Persisted, success.state());
}

#[test]
fn indeterminate_temp_dir_keep_disables_automatic_cleanup() {
    let cleanup_calls = Arc::new(Mutex::new(0));
    {
        let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
        let mut temp = lifecycle_temp_dir(
            fs,
            [],
            Some(FsErrorKind::Indeterminate),
            None,
            cleanup_calls.clone(),
            Arc::new(Mutex::new(0)),
        );

        assert_eq!(FsErrorKind::Indeterminate, temp.keep().unwrap_err().kind(),);
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    assert_eq!(0, *cleanup_calls.lock().expect("lock should succeed"));
}

struct DirectoryNoAtomicFileSystem {
    info: FileSystemInfo,
}

impl FileSystemProperties for DirectoryNoAtomicFileSystem {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits =
            qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for DirectoryNoAtomicFileSystem {
    fn stat(&self, _path: &FsPath) -> qubit_fs::FsResult<FileMetadata> {
        Ok(FileMetadata::new(FileKind::Directory))
    }
}

#[test]
fn temp_dir_required_atomicity_is_rejected_before_provider_persist() {
    let fs: Arc<dyn FileSystem> = Arc::new(DirectoryNoAtomicFileSystem {
        info: FileSystemInfo::new(
            FileSystemId::new("no-atomic-dir").unwrap(),
            ProviderId::new("mock").unwrap(),
            PathSemantics::Hierarchical,
        ),
    });
    let persist_calls = Arc::new(Mutex::new(0));
    let mut temp = lifecycle_temp_dir(
        fs,
        [],
        None,
        None,
        Arc::new(Mutex::new(0)),
        persist_calls.clone(),
    );
    let options = PersistOptions {
        atomicity: AtomicityRequirement::Required,
        ..PersistOptions::default()
    };

    assert!(
        temp.persist(&FsPath::parse("/final").unwrap(), options)
            .is_err(),
    );
    assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
}
