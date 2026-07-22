// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AchievedAtomicity,
    AsyncDirectoryStream,
    AsyncDirectoryStreamSession,
    AsyncFileResource,
    AsyncFileSystem,
    AsyncTempDir,
    AsyncTempFile,
    AsyncTempResourceSession,
    AtomicityRequirement,
    CreateDirOptions,
    DirEntry,
    FileKind,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsFuture,
    FsName,
    FsOperation,
    FsPath,
    ListOptions,
    PathSemantics,
    PersistFailure,
    PersistFailureState,
    PersistFuture,
    PersistOptions,
    PersistOutcome,
    PublicationMethod,
    RelativeFsPath,
    TempResourceState,
};
use qubit_spi::ProviderId;

#[derive(Debug)]
struct AsyncTempFs {
    info: FileSystemInfo,
    atomic_persist: bool,
    limits: FileSystemLimits,
}

impl FileSystemProperties for AsyncTempFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        let capabilities = FileSystemCapabilities::default();
        if self.atomic_persist {
            capabilities.with(FileSystemCapability::AtomicTempPersist)
        } else {
            capabilities
        }
    }

    fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

impl AsyncFileSystem for AsyncTempFs {
    fn stat_async<'a>(
        &'a self,
        _path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata> {
        Box::pin(async { Ok(FileMetadata::new(FileKind::File)) })
    }

    fn list_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ListOptions,
    ) -> FsFuture<'a, AsyncDirectoryStream> {
        let entry = DirEntry::new(path.clone(), FileKind::File);
        Box::pin(async move {
            Ok(AsyncDirectoryStream::new(AsyncTempDirectorySession {
                entry: Some(entry),
            }))
        })
    }

    fn create_dir_async<'a>(
        &'a self,
        _path: &'a FsPath,
        _options: CreateDirOptions,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

struct AsyncTempDirectorySession {
    entry: Option<DirEntry>,
}

impl AsyncDirectoryStreamSession for AsyncTempDirectorySession {
    fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        let entry = self.entry.take();
        Box::pin(async move { Ok(entry) })
    }
}

#[derive(Debug)]
struct AsyncSession {
    fail_once: bool,
    drop_cancellations: Arc<Mutex<usize>>,
}

impl AsyncTempResourceSession for AsyncSession {
    fn cleanup_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn keep_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn persist_async<'a>(
        self: Pin<&'a mut Self>,
        target: &'a FsPath,
        _options: PersistOptions,
    ) -> PersistFuture<'a> {
        let this = self.get_mut();
        if this.fail_once {
            this.fail_once = false;
            return Box::pin(async {
                Err(PersistFailure::new(
                    FsError::new(
                        FsErrorKind::Io,
                        FsOperation::PersistTemp,
                        "not published",
                    ),
                    PersistFailureState::NotPublished,
                ))
            });
        }
        let target = target.clone();
        Box::pin(async move {
            Ok(PersistOutcome::new(
                target,
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            ))
        })
    }

    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .drop_cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

fn assert_pending<F>(mut future: Pin<&mut F>)
where
    F: Future + ?Sized,
{
    let mut context = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
}

fn async_resource_with(
    atomic_persist: bool,
    limits: FileSystemLimits,
) -> AsyncFileResource {
    let fs: Arc<dyn AsyncFileSystem> = Arc::new(AsyncTempFs {
        info: FileSystemInfo::new(
            FileSystemId::new("async-temp").expect("id should parse"),
            ProviderId::new("mock").expect("provider id should parse"),
            PathSemantics::Hierarchical,
        ),
        atomic_persist,
        limits,
    });
    AsyncFileResource::new(
        fs,
        FsPath::parse_normalized("/tmp/source").expect("path should parse"),
    )
}

fn async_resource_with_atomic(atomic_persist: bool) -> AsyncFileResource {
    async_resource_with(atomic_persist, FileSystemLimits::unknown())
}

fn async_resource() -> AsyncFileResource {
    async_resource_with_atomic(true)
}

#[test]
fn async_persist_failure_retains_handle_for_retry() {
    let mut temp = AsyncTempFile::new(
        async_resource(),
        AsyncSession {
            fail_once: true,
            drop_cancellations: Arc::new(Mutex::new(0)),
        },
    );
    let target = FsPath::parse_normalized("/final").expect("path should parse");

    assert!(
        ready(temp.persist_async(&target, PersistOptions::default())).is_err()
    );
    assert_eq!(TempResourceState::Owned, temp.state());
    assert!(
        ready(temp.persist_async(&target, PersistOptions::default())).is_ok()
    );
    assert_eq!(TempResourceState::Persisted, temp.state());
}

#[test]
fn async_drop_uses_only_nonblocking_provider_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let _temp = AsyncTempFile::new(
            async_resource(),
            AsyncSession {
                fail_once: false,
                drop_cancellations: cancellations.clone(),
            },
        );
    }

    assert_eq!(1, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_temp_dir_derives_only_validated_children() {
    let temp = AsyncTempDir::new(
        async_resource(),
        AsyncSession {
            fail_once: false,
            drop_cancellations: Arc::new(Mutex::new(0)),
        },
    );

    assert_eq!(
        "/tmp/source/child",
        temp.child(&FsName::parse("child").expect("name should parse"))
            .path()
            .as_str(),
    );
    assert_eq!(
        "/tmp/source/a/b",
        temp.descendant(
            &RelativeFsPath::parse("a/b").expect("relative path should parse"),
        )
        .path()
        .as_str(),
    );
}

#[derive(Debug)]
struct ConfigurableAsyncSession {
    cleanup_results: VecDeque<Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    persist_failure: Option<PersistFailureState>,
    drop_cancellations: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
}

impl AsyncTempResourceSession for ConfigurableAsyncSession {
    fn cleanup_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        let result = match self.get_mut().cleanup_results.pop_front() {
            Some(Some(kind)) => Err(FsError::new(
                kind,
                FsOperation::CleanupTemp,
                "cleanup failed",
            )),
            _ => Ok(()),
        };
        Box::pin(async move { result })
    }

    fn keep_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        let result = match self.keep_error {
            Some(kind) => {
                Err(FsError::new(kind, FsOperation::KeepTemp, "keep failed"))
            }
            None => Ok(()),
        };
        Box::pin(async move { result })
    }

    fn persist_async<'a>(
        self: Pin<&'a mut Self>,
        target: &'a FsPath,
        _options: PersistOptions,
    ) -> PersistFuture<'a> {
        let this = self.get_mut();
        *this
            .persist_calls
            .lock()
            .expect("persist lock should succeed") += 1;
        let failure_state = this.persist_failure;
        let target = target.clone();
        Box::pin(async move {
            if let Some(state) = failure_state {
                let kind = if state == PersistFailureState::Indeterminate {
                    FsErrorKind::Indeterminate
                } else {
                    FsErrorKind::Io
                };
                Err(PersistFailure::new(
                    FsError::new(
                        kind,
                        FsOperation::PersistTemp,
                        "persist failed",
                    ),
                    state,
                ))
            } else {
                Ok(PersistOutcome::new(
                    target,
                    AchievedAtomicity::Atomic,
                    PublicationMethod::AtomicRename,
                ))
            }
        })
    }

    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .drop_cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

#[derive(Clone, Copy, Debug)]
enum PendingTempOperation {
    Cleanup,
    Keep,
    Persist,
}

#[derive(Debug)]
struct PendingAsyncSession {
    drop_cancellations: Arc<Mutex<usize>>,
}

impl AsyncTempResourceSession for PendingAsyncSession {
    fn cleanup_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(std::future::pending())
    }

    fn keep_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(std::future::pending())
    }

    fn persist_async<'a>(
        self: Pin<&'a mut Self>,
        _target: &'a FsPath,
        _options: PersistOptions,
    ) -> PersistFuture<'a> {
        Box::pin(std::future::pending())
    }

    fn cancel_on_drop(self: Pin<&mut Self>) {
        *self
            .get_mut()
            .drop_cancellations
            .lock()
            .expect("cancellation lock should succeed") += 1;
    }
}

#[test]
fn dropping_polled_pending_async_temp_file_lifecycle_is_indeterminate() {
    let cancellations = Arc::new(Mutex::new(0));
    let target = FsPath::parse("/final").unwrap();
    for operation in [
        PendingTempOperation::Cleanup,
        PendingTempOperation::Keep,
        PendingTempOperation::Persist,
    ] {
        let mut temp = AsyncTempFile::new(
            async_resource(),
            PendingAsyncSession {
                drop_cancellations: cancellations.clone(),
            },
        );
        match operation {
            PendingTempOperation::Cleanup => {
                let mut future = temp.cleanup_async();
                assert_pending(future.as_mut());
                drop(future);
            }
            PendingTempOperation::Keep => {
                let mut future = temp.keep_async();
                assert_pending(future.as_mut());
                drop(future);
            }
            PendingTempOperation::Persist => {
                let mut future =
                    temp.persist_async(&target, PersistOptions::default());
                assert_pending(future.as_mut());
                drop(future);
            }
        }
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn dropping_polled_pending_async_temp_dir_lifecycle_is_indeterminate() {
    let cancellations = Arc::new(Mutex::new(0));
    let target = FsPath::parse("/final").unwrap();
    for operation in [
        PendingTempOperation::Cleanup,
        PendingTempOperation::Keep,
        PendingTempOperation::Persist,
    ] {
        let mut temp = AsyncTempDir::new(
            async_resource(),
            PendingAsyncSession {
                drop_cancellations: cancellations.clone(),
            },
        );
        match operation {
            PendingTempOperation::Cleanup => {
                let mut future = temp.cleanup_async();
                assert_pending(future.as_mut());
                drop(future);
            }
            PendingTempOperation::Keep => {
                let mut future = temp.keep_async();
                assert_pending(future.as_mut());
                drop(future);
            }
            PendingTempOperation::Persist => {
                let mut future =
                    temp.persist_async(&target, PersistOptions::default());
                assert_pending(future.as_mut());
                drop(future);
            }
        }
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn indeterminate_async_temp_keep_disables_drop_cancellation() {
    let cancellations = Arc::new(Mutex::new(0));
    {
        let mut temp = AsyncTempFile::new(
            async_resource(),
            configurable_session(
                [],
                Some(FsErrorKind::Indeterminate),
                None,
                cancellations.clone(),
                Arc::new(Mutex::new(0)),
            ),
        );
        assert_eq!(
            FsErrorKind::Indeterminate,
            ready(temp.keep_async()).unwrap_err().kind(),
        );
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    {
        let mut temp = AsyncTempDir::new(
            async_resource(),
            configurable_session(
                [],
                Some(FsErrorKind::Indeterminate),
                None,
                cancellations.clone(),
                Arc::new(Mutex::new(0)),
            ),
        );
        assert_eq!(
            FsErrorKind::Indeterminate,
            ready(temp.keep_async()).unwrap_err().kind(),
        );
        assert_eq!(TempResourceState::Indeterminate, temp.state());
    }
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

fn configurable_session(
    cleanup_results: impl IntoIterator<Item = Option<FsErrorKind>>,
    keep_error: Option<FsErrorKind>,
    persist_failure: Option<PersistFailureState>,
    drop_cancellations: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
) -> ConfigurableAsyncSession {
    ConfigurableAsyncSession {
        cleanup_results: cleanup_results.into_iter().collect(),
        keep_error,
        persist_failure,
        drop_cancellations,
        persist_calls,
    }
}

#[test]
fn async_temp_file_accessors_open_methods_keep_and_invalid_states_are_explicit()
{
    let cancellations = Arc::new(Mutex::new(0));
    let mut temp = AsyncTempFile::new(
        async_resource(),
        configurable_session(
            [],
            None,
            None,
            cancellations.clone(),
            Arc::new(Mutex::new(0)),
        ),
    );

    assert_eq!(temp.path(), temp.resource().path());
    assert!(format!("{temp:?}").contains("AsyncTempFile"));
    assert!(ready(temp.open_reader_async(Default::default())).is_err());
    assert!(ready(temp.open_writer_async(Default::default())).is_err());
    assert_eq!("/tmp/source", ready(temp.keep_async()).unwrap().as_str());
    assert_eq!(TempResourceState::Kept, temp.state());
    assert!(ready(temp.keep_async()).is_err());
    assert!(ready(temp.cleanup_async()).is_err());
    assert!(
        ready(temp.persist_async(
            &FsPath::parse("/final").unwrap(),
            PersistOptions::default(),
        ))
        .is_err(),
    );
    drop(temp);
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_temp_file_cleanup_failure_states_and_retry_are_preserved() {
    let cancellations = Arc::new(Mutex::new(0));
    let mut retryable = AsyncTempFile::new(
        async_resource(),
        configurable_session(
            [Some(FsErrorKind::Io), None],
            None,
            None,
            cancellations.clone(),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(ready(retryable.cleanup_async()).is_err());
    assert_eq!(TempResourceState::CleanupRequired, retryable.state());
    ready(retryable.cleanup_async()).unwrap();
    assert_eq!(TempResourceState::Cleaned, retryable.state());
    assert!(ready(retryable.cleanup_async()).is_err());

    let mut indeterminate = AsyncTempFile::new(
        async_resource(),
        configurable_session(
            [Some(FsErrorKind::Indeterminate)],
            None,
            None,
            cancellations.clone(),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(ready(indeterminate.cleanup_async()).is_err());
    assert_eq!(TempResourceState::Indeterminate, indeterminate.state());
    drop(indeterminate);
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_temp_file_keep_failure_and_persist_progress_control_drop() {
    let cancellations = Arc::new(Mutex::new(0));
    let target = FsPath::parse("/final").unwrap();

    {
        let mut keep_failure = AsyncTempFile::new(
            async_resource(),
            configurable_session(
                [],
                Some(FsErrorKind::Io),
                None,
                cancellations.clone(),
                Arc::new(Mutex::new(0)),
            ),
        );
        assert!(ready(keep_failure.keep_async()).is_err());
        assert_eq!(TempResourceState::Owned, keep_failure.state());
    }

    let cases = [
        (
            PersistFailureState::NotPublished,
            TempResourceState::Owned,
            true,
        ),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
            true,
        ),
        (
            PersistFailureState::Indeterminate,
            TempResourceState::Indeterminate,
            false,
        ),
    ];
    for (failure_state, resource_state, cancels_on_drop) in cases {
        let before = *cancellations.lock().expect("lock should succeed");
        let mut temp = AsyncTempFile::new(
            async_resource(),
            configurable_session(
                [],
                None,
                Some(failure_state),
                cancellations.clone(),
                Arc::new(Mutex::new(0)),
            ),
        );
        assert_eq!(
            failure_state,
            ready(temp.persist_async(&target, PersistOptions::default()))
                .unwrap_err()
                .state(),
        );
        assert_eq!(resource_state, temp.state());
        drop(temp);
        let after = *cancellations.lock().expect("lock should succeed");
        assert_eq!(before + usize::from(cancels_on_drop), after);
    }
}

#[test]
fn async_temp_file_required_atomicity_fails_before_session_polling() {
    let persist_calls = Arc::new(Mutex::new(0));
    let mut temp = AsyncTempFile::new(
        async_resource_with_atomic(false),
        configurable_session(
            [],
            None,
            None,
            Arc::new(Mutex::new(0)),
            persist_calls.clone(),
        ),
    );
    let options = PersistOptions {
        atomicity: AtomicityRequirement::Required,
        ..PersistOptions::default()
    };

    assert!(
        ready(temp.persist_async(&FsPath::parse("/final").unwrap(), options))
            .is_err(),
    );
    assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
}

#[test]
fn async_temp_dir_all_operations_and_keep_state_are_usable() {
    let cancellations = Arc::new(Mutex::new(0));
    let mut temp = AsyncTempDir::new(
        async_resource(),
        configurable_session(
            [],
            None,
            None,
            cancellations.clone(),
            Arc::new(Mutex::new(0)),
        ),
    );

    assert_eq!(temp.path(), temp.resource().path());
    assert!(format!("{temp:?}").contains("AsyncTempDir"));
    let mut listing = ready(temp.list_async(ListOptions::default())).unwrap();
    assert!(ready(listing.next_entry_async()).unwrap().is_some());
    let child = ready(temp.create_child_dir_async(
        &FsName::parse("created").unwrap(),
        CreateDirOptions::default(),
    ))
    .unwrap();
    assert_eq!("/tmp/source/created", child.path().as_str());

    assert_eq!("/tmp/source", ready(temp.keep_async()).unwrap().as_str());
    assert_eq!(TempResourceState::Kept, temp.state());
    assert!(ready(temp.keep_async()).is_err());
    assert!(ready(temp.cleanup_async()).is_err());
    assert!(
        ready(temp.persist_async(
            &FsPath::parse("/final").unwrap(),
            PersistOptions::default(),
        ))
        .is_err(),
    );
    drop(temp);
    assert_eq!(0, *cancellations.lock().expect("lock should succeed"));
}

#[test]
fn async_temp_dir_cleanup_and_persist_failures_update_recovery_state() {
    let target = FsPath::parse("/final").unwrap();
    let mut retryable = AsyncTempDir::new(
        async_resource(),
        configurable_session(
            [Some(FsErrorKind::Io), None],
            None,
            None,
            Arc::new(Mutex::new(0)),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(ready(retryable.cleanup_async()).is_err());
    assert_eq!(TempResourceState::CleanupRequired, retryable.state());
    ready(retryable.cleanup_async()).unwrap();
    assert_eq!(TempResourceState::Cleaned, retryable.state());

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
        let mut temp = AsyncTempDir::new(
            async_resource(),
            configurable_session(
                [],
                None,
                Some(failure_state),
                Arc::new(Mutex::new(0)),
                Arc::new(Mutex::new(0)),
            ),
        );
        assert!(
            ready(temp.persist_async(&target, PersistOptions::default()))
                .is_err(),
        );
        assert_eq!(resource_state, temp.state());
    }
}

#[test]
fn async_temp_dir_keep_cleanup_indeterminate_success_and_preflight_are_covered()
{
    let target = FsPath::parse("/final").unwrap();
    let mut keep_failure = AsyncTempDir::new(
        async_resource(),
        configurable_session(
            [],
            Some(FsErrorKind::Io),
            None,
            Arc::new(Mutex::new(0)),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(ready(keep_failure.keep_async()).is_err());
    assert_eq!(TempResourceState::Owned, keep_failure.state());

    let mut indeterminate_cleanup = AsyncTempDir::new(
        async_resource(),
        configurable_session(
            [Some(FsErrorKind::Indeterminate)],
            None,
            None,
            Arc::new(Mutex::new(0)),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(ready(indeterminate_cleanup.cleanup_async()).is_err());
    assert_eq!(
        TempResourceState::Indeterminate,
        indeterminate_cleanup.state()
    );

    let mut success = AsyncTempDir::new(
        async_resource(),
        configurable_session(
            [],
            None,
            None,
            Arc::new(Mutex::new(0)),
            Arc::new(Mutex::new(0)),
        ),
    );
    assert!(
        ready(success.persist_async(&target, PersistOptions::default()))
            .is_ok(),
    );
    assert_eq!(TempResourceState::Persisted, success.state());

    let persist_calls = Arc::new(Mutex::new(0));
    let mut no_atomic = AsyncTempDir::new(
        async_resource_with_atomic(false),
        configurable_session(
            [],
            None,
            None,
            Arc::new(Mutex::new(0)),
            persist_calls.clone(),
        ),
    );
    let options = PersistOptions {
        atomicity: AtomicityRequirement::Required,
        ..PersistOptions::default()
    };
    assert!(ready(no_atomic.persist_async(&target, options)).is_err());
    assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
}

struct DefaultAsyncTempSession;

impl AsyncTempResourceSession for DefaultAsyncTempSession {
    fn cleanup_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn keep_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn persist_async<'a>(
        self: Pin<&'a mut Self>,
        target: &'a FsPath,
        _options: PersistOptions,
    ) -> PersistFuture<'a> {
        let target = target.clone();
        Box::pin(async move {
            Ok(PersistOutcome::new(
                target,
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            ))
        })
    }
}

#[test]
fn async_temp_session_default_drop_cancellation_is_a_nonblocking_noop() {
    let _temp = AsyncTempFile::new(async_resource(), DefaultAsyncTempSession);
}

#[test]
fn async_temp_resources_preflight_target_path_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(4));
    let target = FsPath::parse("/final").unwrap();

    for is_directory in [false, true] {
        let persist_calls = Arc::new(Mutex::new(0));
        let session = configurable_session(
            [],
            None,
            None,
            Arc::new(Mutex::new(0)),
            persist_calls.clone(),
        );
        let resource = async_resource_with(true, limits);
        let failure = if is_directory {
            let mut temp = AsyncTempDir::new(resource, session);
            let failure =
                ready(temp.persist_async(&target, PersistOptions::default()))
                    .unwrap_err();
            assert_eq!(Some(temp.path()), failure.error().path());
            failure
        } else {
            let mut temp = AsyncTempFile::new(resource, session);
            let failure =
                ready(temp.persist_async(&target, PersistOptions::default()))
                    .unwrap_err();
            assert_eq!(Some(temp.path()), failure.error().path());
            failure
        };
        assert_eq!(PersistFailureState::NotPublished, failure.state());
        assert_eq!(FsErrorKind::ResourceLimitExceeded, failure.error().kind());
        assert_eq!(Some(&target), failure.error().target());
        assert_eq!(0, *persist_calls.lock().expect("lock should succeed"));
    }
}
