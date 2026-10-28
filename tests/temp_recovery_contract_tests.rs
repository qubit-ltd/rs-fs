// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

mod handle_support;

#[cfg(feature = "async")]
use std::future::Future;
#[cfg(feature = "async")]
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
#[cfg(feature = "async")]
use std::task::Context;
#[cfg(feature = "async")]
use std::task::Poll;
#[cfg(feature = "async")]
use std::task::Waker;

#[cfg(feature = "async")]
use qubit_fs::AsyncFileSystem;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::AchievedAtomicity;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::PublicationMethod;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncFileSystemSpi;
#[cfg(feature = "async")]
use qubit_fs::spi::AsyncTempResourceSpi;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::FileSystemSpi;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncTempDirectory;
#[cfg(feature = "async")]
use qubit_fs::spi::OpenedAsyncTempFile;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
#[cfg(feature = "async")]
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::spi::TempResourceSpi;
use qubit_fs::temp::PersistCleanupState;
use qubit_fs::temp::PersistFailure;
use qubit_fs::temp::PersistFailureState;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::PersistOutcome;
use qubit_fs::temp::TempOptions;
use qubit_fs::temp::TempResourceState;

struct ResidualProbe {
    kind: FileKind,
    cleanup_calls: Arc<AtomicUsize>,
}

impl ResidualProbe {
    fn outcome(&self, target: &Path) -> PersistOutcome {
        PersistOutcome::new(target.clone(), AchievedAtomicity::Atomic, PublicationMethod::Direct)
            .with_cleanup_state(PersistCleanupState::ResidualTemporaryContainer)
    }

    fn keep_target(&self) -> Path {
        Path::parse(if self.kind == FileKind::File {
            "/kept-file"
        } else {
            "/kept-directory"
        })
        .expect("keep target")
    }
}

impl TempResourceSpi for ResidualProbe {
    fn persist(&mut self, request: PersistRequest<'_>) -> Result<PersistOutcome, SpiPersistFailure> {
        Ok(self.outcome(request.target()))
    }

    fn keep(&mut self) -> Result<PersistOutcome, SpiPersistFailure> {
        let target = self.keep_target();
        Ok(self.outcome(&target))
    }

    fn cleanup(&mut self) -> FsResult<()> {
        self.cleanup_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(feature = "async")]
impl AsyncTempResourceSpi for ResidualProbe {
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        Box::pin(async move {
            self.get_mut().cleanup_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }

    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
        Box::pin(async move {
            let this = self.get_mut();
            let target = this.keep_target();
            Ok(this.outcome(&target))
        })
    }

    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
        Box::pin(async move { Ok(self.get_mut().outcome(request.target())) })
    }
}

fn residual_properties() -> ProviderProperties {
    let operations = ProviderOperations::new()
        .with(ProviderOperation::Stat)
        .with(ProviderOperation::CreateTempFile)
        .with(ProviderOperation::CreateTempDirectory);
    let capabilities = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::TempFile)
        .with_guaranteed(FileSystemCapability::TempDirectory)
        .with_guaranteed(FileSystemCapability::AtomicTempPersist);
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("residual-probe").expect("filesystem id"),
            "residual-probe",
            PathSemantics::Hierarchical,
        ),
        operations,
        capabilities,
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("valid provider properties")
}

struct ResidualProvider {
    properties: ProviderProperties,
    cleanup_calls: Arc<AtomicUsize>,
}

impl ResidualProvider {
    fn opened(&self, kind: FileKind) -> OpenedFileInfo {
        OpenedFileInfo::new(
            self.properties.info().id().clone(),
            Path::parse(if kind == FileKind::File {
                "/scratch-file"
            } else {
                "/scratch-dir"
            })
            .expect("temporary path"),
        )
        .with_metadata(FileMetadata::new(kind))
    }

    fn session(&self, kind: FileKind) -> Box<ResidualProbe> {
        Box::new(ResidualProbe {
            kind,
            cleanup_calls: Arc::clone(&self.cleanup_calls),
        })
    }
}

impl FileSystemSpi for ResidualProvider {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        unreachable!("stat is not used by this contract probe")
    }

    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Ok(OpenedTempFile::new(
            self.opened(FileKind::File),
            self.session(FileKind::File),
        ))
    }

    fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Ok(OpenedTempDirectory::new(
            self.opened(FileKind::Directory),
            self.session(FileKind::Directory),
        ))
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for ResidualProvider {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { unreachable!("stat is not used by this contract probe") })
    }

    fn create_temp_file<'a>(&'a self, _: CreateTempFileRequest) -> SpiFuture<'a, FsResult<OpenedAsyncTempFile>> {
        Box::pin(async move {
            Ok(OpenedAsyncTempFile::new(
                self.opened(FileKind::File),
                self.session(FileKind::File),
            ))
        })
    }

    fn create_temp_directory<'a>(
        &'a self,
        _: CreateTempDirectoryRequest,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncTempDirectory>> {
        Box::pin(async move {
            Ok(OpenedAsyncTempDirectory::new(
                self.opened(FileKind::Directory),
                self.session(FileKind::Directory),
            ))
        })
    }
}

#[cfg(feature = "async")]
fn poll_ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("probe future unexpectedly suspended"),
    }
}

/// A residual private container does not change the confirmed publication
/// result.
#[test]
fn test_persist_outcome_preserves_residual_temporary_container() {
    let target = Path::parse("/published").expect("target");
    let outcome = PersistOutcome::new(target.clone(), AchievedAtomicity::Atomic, PublicationMethod::Direct)
        .with_cleanup_state(PersistCleanupState::ResidualTemporaryContainer);

    assert_eq!(&target, outcome.target());
    assert_eq!(AchievedAtomicity::Atomic, outcome.atomicity());
    assert_eq!(PublicationMethod::Direct, outcome.method());
    assert_eq!(PersistCleanupState::ResidualTemporaryContainer, outcome.cleanup_state());
}

#[test]
fn test_sync_temp_handles_preserve_residual_persist_and_keep_outcomes() {
    for kind in [FileKind::File, FileKind::Directory] {
        for persist in [true, false] {
            let cleanup_calls = Arc::new(AtomicUsize::new(0));
            let provider = ResidualProvider {
                properties: residual_properties(),
                cleanup_calls: Arc::clone(&cleanup_calls),
            };
            let filesystem = FileSystem::from_spi(provider).expect("filesystem");
            let requested_target = Path::parse("/published").expect("target");
            let (outcome, state) = match (kind.clone(), persist) {
                (FileKind::File, true) => {
                    let mut handle = filesystem
                        .create_temp_file(TempOptions::default())
                        .expect("temporary file");
                    let outcome = handle
                        .persist(&requested_target, PersistOptions::default())
                        .expect("persist");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        handle.cleanup().expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::File, false) => {
                    let mut handle = filesystem
                        .create_temp_file(TempOptions::default())
                        .expect("temporary file");
                    let outcome = handle.keep().expect("keep");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        handle.cleanup().expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::Directory, true) => {
                    let mut handle = filesystem
                        .create_temp_directory(TempOptions::default())
                        .expect("temporary directory");
                    let outcome = handle
                        .persist(&requested_target, PersistOptions::default())
                        .expect("persist");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        handle.cleanup().expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::Directory, false) => {
                    let mut handle = filesystem
                        .create_temp_directory(TempOptions::default())
                        .expect("temporary directory");
                    let outcome = handle.keep().expect("keep");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        handle.cleanup().expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                _ => unreachable!(),
            };
            let expected_target = if persist {
                requested_target
            } else if kind == FileKind::File {
                Path::parse("/kept-file").expect("target")
            } else {
                Path::parse("/kept-directory").expect("target")
            };
            assert_eq!(&expected_target, outcome.target());
            assert_eq!(
                if persist {
                    TempResourceState::Persisted
                } else {
                    TempResourceState::Kept
                },
                state
            );
            assert_eq!(PersistCleanupState::ResidualTemporaryContainer, outcome.cleanup_state());
            assert_eq!(0, cleanup_calls.load(Ordering::SeqCst));
        }
    }
}

#[test]
#[cfg(feature = "async")]
fn test_async_temp_handles_preserve_residual_persist_and_keep_outcomes() {
    for kind in [FileKind::File, FileKind::Directory] {
        for persist in [true, false] {
            let cleanup_calls = Arc::new(AtomicUsize::new(0));
            let provider = ResidualProvider {
                properties: residual_properties(),
                cleanup_calls: Arc::clone(&cleanup_calls),
            };
            let filesystem = AsyncFileSystem::from_spi(provider).expect("filesystem");
            let requested_target = Path::parse("/published").expect("target");
            let (outcome, state) = match (kind.clone(), persist) {
                (FileKind::File, true) => {
                    let mut handle =
                        poll_ready(filesystem.create_temp_file(TempOptions::default())).expect("temporary file");
                    let outcome =
                        poll_ready(handle.persist(&requested_target, PersistOptions::default())).expect("persist");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        poll_ready(handle.cleanup()).expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::File, false) => {
                    let mut handle =
                        poll_ready(filesystem.create_temp_file(TempOptions::default())).expect("temporary file");
                    let outcome = poll_ready(handle.keep()).expect("keep");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        poll_ready(handle.cleanup()).expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::Directory, true) => {
                    let mut handle = poll_ready(filesystem.create_temp_directory(TempOptions::default()))
                        .expect("temporary directory");
                    let outcome =
                        poll_ready(handle.persist(&requested_target, PersistOptions::default())).expect("persist");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        poll_ready(handle.cleanup()).expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                (FileKind::Directory, false) => {
                    let mut handle = poll_ready(filesystem.create_temp_directory(TempOptions::default()))
                        .expect("temporary directory");
                    let outcome = poll_ready(handle.keep()).expect("keep");
                    let state = handle.state();
                    assert_eq!(
                        FsErrorKind::InvalidState,
                        poll_ready(handle.cleanup()).expect_err("terminal cleanup").kind()
                    );
                    (outcome, state)
                }
                _ => unreachable!(),
            };
            let expected_target = if persist {
                requested_target
            } else if kind == FileKind::File {
                Path::parse("/kept-file").expect("target")
            } else {
                Path::parse("/kept-directory").expect("target")
            };
            assert_eq!(&expected_target, outcome.target());
            assert_eq!(
                if persist {
                    TempResourceState::Persisted
                } else {
                    TempResourceState::Kept
                },
                state
            );
            assert_eq!(PersistCleanupState::ResidualTemporaryContainer, outcome.cleanup_state());
            assert_eq!(0, cleanup_calls.load(Ordering::SeqCst));
        }
    }
}

#[test]
fn test_persist_failure_retains_release_state() {
    let failure = PersistFailure::new(
        FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "injected"),
        PersistFailureState::PublishedSourceReleased,
    );
    assert_eq!(None, failure.publication_target());
    let (_, state, restored_target) = failure.into_recovery_parts();
    assert_eq!(PersistFailureState::PublishedSourceReleased, state);
    assert_eq!(None, restored_target);
}

/// Failed unpublished attempts must not fabricate historical publication.
#[test]
fn test_unpublished_failure_has_no_publication_target() {
    let (filesystem, _, _) =
        handle_support::implementation_tests::temp_failure_filesystem(PersistFailureState::NotPublished);
    let target = Path::parse("/requested").expect("target");
    let mut file = filesystem.create_temp_file(TempOptions::default()).expect("file");
    let failure = file.persist(&target, PersistOptions::default()).expect_err("failure");
    assert_eq!(None, failure.publication_target());
    file.cleanup().expect("cleanup");
    let retry = file.keep().expect_err("cleaned");
    assert_eq!(PersistFailureState::NotPublishedSourceReleased, retry.state());
    assert_eq!(None, retry.publication_target());
}

/// Source uncertainty cannot regain mutation authority through invalid retries.
#[test]
fn test_temp_file_source_failure_states_are_sticky() {
    for (state, expected, published) in [
        (
            PersistFailureState::NotPublishedSourceIndeterminate,
            TempResourceState::Indeterminate,
            false,
        ),
        (
            PersistFailureState::PublishedSourceIndeterminate,
            TempResourceState::Indeterminate,
            true,
        ),
        (
            PersistFailureState::NotPublishedSourceCleanupRequired,
            TempResourceState::CleanupRequired,
            false,
        ),
    ] {
        let (filesystem, cleanup_calls, persist_calls) =
            handle_support::implementation_tests::temp_failure_filesystem(state);
        let mut temporary = filesystem.create_temp_file(TempOptions::default()).expect("resource");
        let target = Path::parse("/published").expect("target");
        let failure = temporary
            .persist(&target, PersistOptions::default())
            .expect_err("injected failure");
        assert_eq!(state, failure.state());
        assert_eq!(expected, temporary.state());
        assert_eq!(published.then_some(&target), failure.publication_target());
        let retry = temporary
            .persist(&Path::parse("relative").expect("relative"), PersistOptions::default())
            .expect_err("not owned");
        assert_eq!(FsErrorKind::InvalidState, retry.error().kind());
        assert_eq!(state, retry.state());
        assert_eq!(published.then_some(&target), retry.publication_target());
        let keep = temporary.keep().expect_err("not owned");
        assert_eq!(state, keep.state());
        assert_eq!(published.then_some(&target), keep.publication_target());
        if expected == TempResourceState::Indeterminate {
            assert_eq!(
                FsErrorKind::InvalidState,
                temporary.cleanup().expect_err("uncertain source").kind()
            );
            assert_eq!(expected, temporary.state());
        } else {
            temporary.cleanup().expect("remaining sandbox cleanup");
            let retry = temporary.keep().expect_err("cleaned");
            assert_eq!(PersistFailureState::NotPublishedSourceReleased, retry.state());
            assert_eq!(None, retry.publication_target());
        }
        drop(temporary);
        assert_eq!(1, *persist_calls.lock().expect("calls"));
        assert_eq!(
            usize::from(expected == TempResourceState::CleanupRequired),
            *cleanup_calls.lock().expect("calls")
        );
    }
}

/// Source uncertainty cannot regain mutation authority through invalid retries.
#[test]
fn test_temp_directory_source_failure_states_are_sticky() {
    for (state, expected, published) in [
        (
            PersistFailureState::NotPublishedSourceIndeterminate,
            TempResourceState::Indeterminate,
            false,
        ),
        (
            PersistFailureState::PublishedSourceIndeterminate,
            TempResourceState::Indeterminate,
            true,
        ),
        (
            PersistFailureState::NotPublishedSourceCleanupRequired,
            TempResourceState::CleanupRequired,
            false,
        ),
    ] {
        let (filesystem, cleanup_calls, persist_calls) =
            handle_support::implementation_tests::temp_failure_filesystem(state);
        let mut temporary = filesystem
            .create_temp_directory(TempOptions::default())
            .expect("resource");
        let target = Path::parse("/published").expect("target");
        let failure = temporary
            .persist(&target, PersistOptions::default())
            .expect_err("injected failure");
        assert_eq!(state, failure.state());
        assert_eq!(expected, temporary.state());
        assert_eq!(published.then_some(&target), failure.publication_target());
        let retry = temporary
            .persist(&Path::parse("relative").expect("relative"), PersistOptions::default())
            .expect_err("not owned");
        assert_eq!(FsErrorKind::InvalidState, retry.error().kind());
        assert_eq!(state, retry.state());
        assert_eq!(published.then_some(&target), retry.publication_target());
        let keep = temporary.keep().expect_err("not owned");
        assert_eq!(state, keep.state());
        assert_eq!(published.then_some(&target), keep.publication_target());
        if expected == TempResourceState::Indeterminate {
            assert_eq!(
                FsErrorKind::InvalidState,
                temporary.cleanup().expect_err("uncertain source").kind()
            );
            assert_eq!(expected, temporary.state());
        } else {
            temporary.cleanup().expect("remaining sandbox cleanup");
            let retry = temporary.keep().expect_err("cleaned");
            assert_eq!(PersistFailureState::NotPublishedSourceReleased, retry.state());
            assert_eq!(None, retry.publication_target());
        }
        drop(temporary);
        assert_eq!(1, *persist_calls.lock().expect("calls"));
        assert_eq!(
            usize::from(expected == TempResourceState::CleanupRequired),
            *cleanup_calls.lock().expect("calls")
        );
    }
}

/// Keep failures retain the generated destination rather than the source path.
#[test]
fn test_temp_file_keep_failure_retains_generated_target() {
    for state in [
        PersistFailureState::PublishedSourceIndeterminate,
        PersistFailureState::PublishedSourceRetained,
        PersistFailureState::PublishedSourceReleased,
    ] {
        let (filesystem, _, _) = handle_support::implementation_tests::temp_failure_filesystem(state);
        let mut temporary = filesystem.create_temp_file(TempOptions::default()).expect("resource");
        let target = Path::parse("/kept-resource").expect("target");
        let failure = temporary.keep().expect_err("injected keep failure");
        assert_eq!(Some(&target), failure.publication_target());
        assert_eq!(state, failure.state());
        if state == PersistFailureState::PublishedSourceReleased {
            assert_eq!(TempResourceState::Kept, temporary.state());
        }
        if state == PersistFailureState::PublishedSourceRetained {
            temporary.cleanup().expect("cleanup");
        }
        let retry = temporary.keep().expect_err("not owned");
        assert_eq!(Some(&target), retry.publication_target());
        assert_eq!(
            if state == PersistFailureState::PublishedSourceRetained {
                PersistFailureState::PublishedSourceReleased
            } else {
                state
            },
            retry.state()
        );
    }
}

/// Keep failures retain the generated destination rather than the source path.
#[test]
fn test_temp_directory_keep_failure_retains_generated_target() {
    for state in [
        PersistFailureState::PublishedSourceIndeterminate,
        PersistFailureState::PublishedSourceRetained,
        PersistFailureState::PublishedSourceReleased,
    ] {
        let (filesystem, _, _) = handle_support::implementation_tests::temp_failure_filesystem(state);
        let mut temporary = filesystem
            .create_temp_directory(TempOptions::default())
            .expect("resource");
        let target = Path::parse("/kept-resource").expect("target");
        let failure = temporary.keep().expect_err("injected keep failure");
        assert_eq!(Some(&target), failure.publication_target());
        assert_eq!(state, failure.state());
        if state == PersistFailureState::PublishedSourceReleased {
            assert_eq!(TempResourceState::Kept, temporary.state());
        }
        if state == PersistFailureState::PublishedSourceRetained {
            temporary.cleanup().expect("cleanup");
        }
        let retry = temporary.keep().expect_err("not owned");
        assert_eq!(Some(&target), retry.publication_target());
        assert_eq!(
            if state == PersistFailureState::PublishedSourceRetained {
                PersistFailureState::PublishedSourceReleased
            } else {
                state
            },
            retry.state()
        );
    }
}

/// Cleanup failures retain their source qualification in later rejected calls.
#[test]
fn test_cleanup_failure_does_not_restore_owned_source() {
    for (kind, expected) in [
        (FsErrorKind::Io, PersistFailureState::NotPublishedSourceCleanupRequired),
        (
            FsErrorKind::Indeterminate,
            PersistFailureState::NotPublishedSourceIndeterminate,
        ),
    ] {
        let (filesystem, calls) =
            handle_support::implementation_tests::temp_lifecycle_error_filesystem(None, Some(kind));
        let mut file = filesystem.create_temp_file(TempOptions::default()).expect("file");
        assert_eq!(kind, file.cleanup().expect_err("injected cleanup failure").kind());
        let retry = file.keep().expect_err("not owned");
        assert_eq!(expected, retry.state());
        assert_eq!(None, retry.publication_target());
        assert_eq!(1, *calls.lock().expect("cleanup calls"));
    }
}

/// A cleanup uncertainty after publication preserves the known target
/// permanently.
#[test]
fn test_temp_file_published_cleanup_becomes_source_indeterminate() {
    let (filesystem, cleanup_calls, persist_calls) =
        handle_support::implementation_tests::temp_failure_with_cleanup_error_filesystem(
            PersistFailureState::PublishedSourceRetained,
            Some(FsErrorKind::Indeterminate),
        );
    let mut temporary = filesystem.create_temp_file(TempOptions::default()).expect("resource");
    let target = Path::parse("/published").expect("target");
    let failure = temporary
        .persist(&target, PersistOptions::default())
        .expect_err("partial publication");
    assert_eq!(PersistFailureState::PublishedSourceRetained, failure.state());
    assert_eq!(TempResourceState::CleanupRequired, temporary.state());
    assert_eq!(Some(&target), failure.publication_target());
    let cleanup = temporary.cleanup().expect_err("uncertain cleanup");
    assert_eq!(FsErrorKind::Indeterminate, cleanup.kind());
    assert_eq!(TempResourceState::Indeterminate, temporary.state());
    let retry = temporary
        .persist(&Path::parse("relative").expect("relative"), PersistOptions::default())
        .expect_err("source authority is uncertain");
    assert_eq!(FsErrorKind::InvalidState, retry.error().kind());
    assert_eq!(PersistFailureState::PublishedSourceIndeterminate, retry.state());
    assert_eq!(Some(&target), retry.publication_target());
    let keep = temporary.keep().expect_err("source authority is uncertain");
    assert_eq!(PersistFailureState::PublishedSourceIndeterminate, keep.state());
    assert_eq!(Some(&target), keep.publication_target());
    assert_eq!(
        FsErrorKind::InvalidState,
        temporary.cleanup().expect_err("no cleanup retry").kind()
    );
    assert_eq!(TempResourceState::Indeterminate, temporary.state());
    drop(temporary);
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup calls"));
    assert_eq!(1, *persist_calls.lock().expect("persist calls"));
}

/// A cleanup uncertainty after publication preserves the known target
/// permanently.
#[test]
fn test_temp_directory_published_cleanup_becomes_source_indeterminate() {
    let (filesystem, cleanup_calls, persist_calls) =
        handle_support::implementation_tests::temp_failure_with_cleanup_error_filesystem(
            PersistFailureState::PublishedSourceRetained,
            Some(FsErrorKind::Indeterminate),
        );
    let mut temporary = filesystem
        .create_temp_directory(TempOptions::default())
        .expect("resource");
    let target = Path::parse("/published").expect("target");
    let failure = temporary
        .persist(&target, PersistOptions::default())
        .expect_err("partial publication");
    assert_eq!(PersistFailureState::PublishedSourceRetained, failure.state());
    assert_eq!(TempResourceState::CleanupRequired, temporary.state());
    assert_eq!(Some(&target), failure.publication_target());
    let cleanup = temporary.cleanup().expect_err("uncertain cleanup");
    assert_eq!(FsErrorKind::Indeterminate, cleanup.kind());
    assert_eq!(TempResourceState::Indeterminate, temporary.state());
    let retry = temporary
        .persist(&Path::parse("relative").expect("relative"), PersistOptions::default())
        .expect_err("source authority is uncertain");
    assert_eq!(FsErrorKind::InvalidState, retry.error().kind());
    assert_eq!(PersistFailureState::PublishedSourceIndeterminate, retry.state());
    assert_eq!(Some(&target), retry.publication_target());
    let keep = temporary.keep().expect_err("source authority is uncertain");
    assert_eq!(PersistFailureState::PublishedSourceIndeterminate, keep.state());
    assert_eq!(Some(&target), keep.publication_target());
    assert_eq!(
        FsErrorKind::InvalidState,
        temporary.cleanup().expect_err("no cleanup retry").kind()
    );
    assert_eq!(TempResourceState::Indeterminate, temporary.state());
    drop(temporary);
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup calls"));
    assert_eq!(1, *persist_calls.lock().expect("persist calls"));
}
