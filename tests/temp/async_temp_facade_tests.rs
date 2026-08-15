// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External SPI-bound temporary resource behavior tests.

use std::error::Error;
use std::pin::Pin;

use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::path::PathComponent;
use qubit_fs::path::RelativePath;
use qubit_fs::spi::AsyncTempResourceSpi;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::temp::PersistFailureState;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::PersistOutcome;
use qubit_fs::temp::TempOptions;
use qubit_fs::temp::TempResourceState;

use crate::async_recording_spi::AsyncRecordingConfig;
use crate::async_recording_spi::async_recording_file_system;
use crate::poll_support::ready;

/// Returns a stable absolute destination used by persistence tests.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Minimal resource session used to invoke the default drop-cancellation hook.
struct DefaultTempResource;

impl AsyncTempResourceSpi for DefaultTempResource {
    /// This test session never performs provider cleanup.
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        unimplemented!("cleanup is outside this default-hook test")
    }

    /// This test session never transfers cleanup responsibility.
    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
        unimplemented!("keep is outside this default-hook test")
    }

    /// This test session never persists a temporary resource.
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
        let _ = request;
        unimplemented!("persist is outside this default-hook test")
    }
}

/// Rejects a provider-created temporary identity before a handle escapes the
/// facade.
#[test]
fn test_async_temp_creation_rejects_mismatched_provider_identity() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            invalid_temp_identity: true,
            ..AsyncRecordingConfig::default()
        });
    let Err(error) =
        ready(file_system.create_temp_file(TempOptions::default()))
    else {
        panic!("mismatched temporary identity must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(vec!["create_temp_file", "cleanup"], probe.calls());
}

/// Verifies the default temporary-resource drop hook is a nonblocking no-op.
#[test]
fn test_async_temp_resource_default_cancel_on_drop_is_noop() {
    let mut resource = DefaultTempResource;
    Pin::new(&mut resource).cancel_on_drop();
}

/// Verifies async temporary options are validated before provider creation.
#[test]
fn test_async_temp_creation_validates_parent_before_provider_call() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let parent = path("relative");
    let error = match ready(file_system.create_temp_file(
        TempOptions::default().with_parent(Some(parent.clone())),
    )) {
        Ok(_) => panic!("invalid temporary parent must fail in the facade"),
        Err(error) => error,
    };
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::CreateTemp, error.operation());
    assert_eq!(Some(&parent), error.path());
    assert!(
        probe.calls().is_empty(),
        "provider creation must not be called"
    );
}

/// Verifies async pathless provider failures omit fabricated root context.
#[test]
fn test_async_temp_creation_keeps_provider_error_pathless() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            temp_create_error: true,
            ..AsyncRecordingConfig::default()
        });
    let error =
        match ready(file_system.create_temp_file(TempOptions::default())) {
            Ok(_) => panic!("provider creation failure should propagate"),
            Err(error) => error,
        };
    assert_eq!(FsOperation::CreateTemp, error.operation());
    assert_eq!(None, error.path());
    assert_eq!(Some("async-recording"), error.provider());
    assert_eq!(vec!["create_temp_file"], probe.calls());
}

/// Applies the same identity and compensating-cleanup contract to temporary
/// directories as it does to temporary files.
#[test]
fn test_async_temp_directory_rejects_mismatched_provider_identity() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            invalid_temp_identity: true,
            ..AsyncRecordingConfig::default()
        });
    let Err(error) =
        ready(file_system.create_temp_directory(TempOptions::default()))
    else {
        panic!("mismatched temporary directory identity must be rejected");
    };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(vec!["create_temp_directory", "cleanup"], probe.calls());
}

/// Verifies failed invalid-session cleanup remains the inspectable source of
/// the facade contract failure.
#[test]
fn test_async_invalid_temp_identity_preserves_cleanup_error_source() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_temp_identity: true,
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let Err(error) =
        ready(file_system.create_temp_file(TempOptions::default()))
    else {
        panic!("invalid temporary identity must fail");
    };
    let cleanup = error.source().expect("cleanup error should be retained");
    assert!(cleanup.to_string().contains("injected cleanup failure"));
    assert!(cleanup.source().is_some());
}

/// Rejects an asynchronous temporary-file envelope whose metadata claims a
/// directory.
#[test]
fn test_async_temp_creation_rejects_wrong_kind() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            invalid_temp_kind: true,
            ..AsyncRecordingConfig::default()
        });
    let error =
        match ready(file_system.create_temp_file(TempOptions::default())) {
            Ok(_) => panic!("temporary-file kind must be validated"),
            Err(error) => error,
        };
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(vec!["create_temp_file", "cleanup"], probe.calls());
}

/// Mirrors synchronous temporary-directory path helpers in the async facade.
#[test]
fn test_async_temp_directory_builds_child_and_descendant_paths() {
    let (file_system, _) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let directory =
        ready(file_system.create_temp_directory(TempOptions::default()))
            .expect("temporary directory should open");
    let component = PathComponent::parse("child").expect("component parses");
    let descendant =
        RelativePath::parse("nested/item").expect("relative path parses");
    assert_eq!("/tmp/recording/child", directory.child(&component).as_str());
    assert_eq!(
        "/tmp/recording/nested/item",
        directory.descendant(&descendant).as_str()
    );
}

/// Verifies persistence preflight fails before calling a temporary session.
#[test]
fn test_async_temp_persist_preflight_has_no_session_call() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut temp = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");
    let failure = ready(
        temp.persist(
            &path("/final"),
            PersistOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        ),
    )
    .expect_err("missing atomic persistence support must fail locally");
    assert_eq!(FsErrorKind::RequirementNotMet, failure.error().kind());
    assert_eq!(TempResourceState::Owned, temp.state());
    assert_eq!(vec!["create_temp_file"], probe.calls());
}

/// Covers successful temporary file persistence and terminal state publication.
#[test]
fn test_async_temp_file_persist_delegates_after_preflight() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            atomic_temp_persist: true,
            ..AsyncRecordingConfig::default()
        });
    let mut temp = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");
    let outcome =
        ready(temp.persist(&path("/final"), PersistOptions::default()))
            .expect("persistence should succeed");
    assert_eq!(&path("/final"), outcome.target());
    assert_eq!(TempResourceState::Persisted, temp.state());
    assert_eq!(vec!["create_temp_file", "persist"], probe.calls());
}

/// Rejects a provider outcome that reports a different persistence target.
#[test]
fn test_async_temp_file_rejects_mismatched_persist_target() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        ..AsyncRecordingConfig::default()
    });
    let mut temp = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");

    let failure = ready(
        temp.persist(&path("/wrong-persist-target"), PersistOptions::default()),
    )
    .expect_err("mismatched target should violate the provider contract");

    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(PersistFailureState::Indeterminate, failure.state());
    assert_eq!(TempResourceState::Indeterminate, temp.state());
}

/// Covers cleanup and keep delegation for both opaque temporary resource kinds.
#[test]
fn test_async_temp_cleanup_and_keep_delegate_to_spi_sessions() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    let mut file = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");
    ready(file.cleanup()).expect("cleanup should succeed");
    assert_eq!(TempResourceState::Cleaned, file.state());
    let mut directory =
        ready(file_system.create_temp_directory(TempOptions::default()))
            .expect("temporary directory should open");
    ready(directory.keep()).expect("keep should succeed");
    assert_eq!(TempResourceState::Kept, directory.state());
    assert_eq!(
        vec![
            "create_temp_file",
            "cleanup",
            "create_temp_directory",
            "keep"
        ],
        probe.calls()
    );
}

/// Preserves indeterminate persistence instead of claiming cleanup is required.
#[test]
fn test_async_temp_persist_indeterminate_is_preserved() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        temp_persist_indeterminate: true,
        ..AsyncRecordingConfig::default()
    });
    let mut temp = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");
    let error = ready(temp.persist(&path("/final"), PersistOptions::default()))
        .expect_err("persistence should be indeterminate");
    assert_eq!(FsErrorKind::Indeterminate, error.error().kind());
    assert_eq!(Some(&path("/tmp/recording")), error.error().path(),);
    assert_eq!(Some(&path("/final")), error.error().target());
    assert_eq!(Some("async-recording"), error.error().provider());
    assert_eq!(TempResourceState::Indeterminate, temp.state());
}

/// Keeps definite lifecycle failures recoverable and makes cleanup failure
/// require a later explicit cleanup decision.
#[test]
fn test_async_temp_lifecycle_failures_preserve_expected_states() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        temp_keep_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let mut file = ready(file_system.create_temp_file(TempOptions::default()))
        .expect("temporary file should open");
    let keep = ready(file.keep())
        .expect_err("configured keep failure should propagate");
    assert_eq!(FsErrorKind::Io, keep.kind());
    assert_eq!(FsOperation::KeepTemp, keep.operation());
    assert_eq!(Some(&path("/tmp/recording")), keep.path());
    assert_eq!(Some("async-recording"), keep.provider());
    assert_eq!(TempResourceState::Owned, file.state());

    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        temp_cleanup_failure: true,
        ..AsyncRecordingConfig::default()
    });
    let mut directory =
        ready(file_system.create_temp_directory(TempOptions::default()))
            .expect("temporary directory should open");
    let cleanup = ready(directory.cleanup())
        .expect_err("configured cleanup failure should propagate");
    assert_eq!(FsErrorKind::Io, cleanup.kind());
    assert_eq!(FsOperation::CleanupTemp, cleanup.operation());
    assert_eq!(Some(&path("/tmp/recording")), cleanup.path());
    assert_eq!(Some("async-recording"), cleanup.provider());
    assert_eq!(TempResourceState::CleanupRequired, directory.state());
}

/// Exposes the temporary-directory wrapper's path, persistence, and terminal
/// lifecycle validation through the public asynchronous facade.
#[test]
fn test_async_temp_directory_persists_and_rejects_later_lifecycle_calls() {
    let (file_system, _) = async_recording_file_system(AsyncRecordingConfig {
        atomic_temp_persist: true,
        ..AsyncRecordingConfig::default()
    });
    let mut directory =
        ready(file_system.create_temp_directory(TempOptions::default()))
            .expect("temporary directory should open");
    assert_eq!(&path("/tmp/recording"), directory.path());
    let outcome =
        ready(directory.persist(&path("/final"), PersistOptions::default()))
            .expect("temporary directory should persist");
    assert_eq!(&path("/final"), outcome.target());
    assert_eq!(TempResourceState::Persisted, directory.state());
    let persist =
        ready(directory.persist(&path("/other"), PersistOptions::default()))
            .expect_err("persisted directory must reject a second persist");
    assert_eq!(FsErrorKind::InvalidState, persist.error().kind());
    assert!(persist.error().to_string().contains("temporary directory"));
    let cleanup = ready(directory.cleanup())
        .expect_err("persisted directory must reject later cleanup");
    assert_eq!(FsErrorKind::InvalidState, cleanup.kind());
    assert!(cleanup.to_string().contains("temporary directory"));
}

/// Notifies providers about local cancellation for owned and recoverable
/// temporary resources without attempting asynchronous cleanup in `Drop`.
#[test]
fn test_async_temp_drop_notifies_provider_for_cleanup_owned_states() {
    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig::default());
    {
        let _file = ready(file_system.create_temp_file(TempOptions::default()))
            .expect("temporary file should open");
    }
    assert_eq!(1, probe.temp_cancellations());

    let (file_system, probe) =
        async_recording_file_system(AsyncRecordingConfig {
            atomic_temp_persist: true,
            temp_persist_failure: Some(
                PersistFailureState::PublishedSourceRetained,
            ),
            ..AsyncRecordingConfig::default()
        });
    {
        let mut directory =
            ready(file_system.create_temp_directory(TempOptions::default()))
                .expect("temporary directory should open");
        let failure = ready(
            directory.persist(&path("/final"), PersistOptions::default()),
        )
        .expect_err("configured persistence failure should propagate");
        assert_eq!(
            PersistFailureState::PublishedSourceRetained,
            failure.state()
        );
        assert_eq!(TempResourceState::CleanupRequired, directory.state());
    }
    assert_eq!(1, probe.temp_cancellations());
}

/// Retains the provider-confirmed state after both definite and partially
/// published temporary persistence failures.
#[test]
fn test_async_temp_persist_failure_states_drive_lifecycle() {
    for (failure_state, expected_state) in [
        (PersistFailureState::NotPublished, TempResourceState::Owned),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
        ),
    ] {
        let (file_system, _) =
            async_recording_file_system(AsyncRecordingConfig {
                atomic_temp_persist: true,
                temp_persist_failure: Some(failure_state),
                ..AsyncRecordingConfig::default()
            });
        let mut file =
            ready(file_system.create_temp_file(TempOptions::default()))
                .expect("temporary file should open");
        let failure =
            ready(file.persist(&path("/final"), PersistOptions::default()))
                .expect_err("configured persist failure should propagate");
        assert_eq!(failure_state, failure.state());
        assert_eq!(FsErrorKind::Io, failure.error().kind());
        assert_eq!(expected_state, file.state());
    }
}
