// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::Path;
use qubit_fs::PathComponent;
use qubit_fs::PersistFailureState;
use qubit_fs::PersistOptions;
use qubit_fs::RelativePath;
use qubit_fs::TempDirectoryOptions;
use qubit_fs::TempResourceState;
#[test]
fn test_temp_directory_child_components_are_lexically_safe() {
    let (filesystem, cleanup_calls, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");
    assert_eq!("/temporary", directory.path().as_str());
    let component =
        PathComponent::parse("child").expect("component should parse");
    assert_eq!("/temporary/child", directory.child(&component).as_str());
    let descendant = RelativePath::parse("nested/child")
        .expect("relative path should parse");
    assert_eq!(
        "/temporary/nested/child",
        directory.descendant(&descendant).as_str()
    );
    assert!(format!("{directory:?}").contains("TempDirectory"));
    directory.cleanup().expect("cleanup should succeed");
    assert_eq!(TempResourceState::Cleaned, directory.state());
    assert_eq!(
        1,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}

/// Verifies a successful directory persist releases the owned source and
/// prevents subsequent lifecycle actions from reusing the session.
#[test]
fn test_temp_directory_persist_marks_resource_persisted() {
    let (filesystem, cleanup_calls, persist_calls) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");
    let target =
        Path::parse("/published-directory").expect("target should parse");

    let outcome = directory
        .persist(&target, PersistOptions::default())
        .expect("atomic temporary directory persist should succeed");
    assert_eq!(&target, outcome.target());
    assert_eq!(TempResourceState::Persisted, directory.state());
    assert_eq!(
        1,
        *persist_calls.lock().expect("persist lock should succeed")
    );
    assert!(directory.cleanup().is_err());
    assert!(directory.keep().is_err());
    drop(directory);
    assert_eq!(
        0,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}

/// Verifies a provider cannot satisfy required atomic directory persistence by
/// merely advertising the capability and returning a non-atomic outcome.
#[test]
fn test_temp_directory_rejects_non_atomic_required_persist_outcome() {
    let filesystem =
        crate::handle_support::non_atomic_temp_directory_filesystem();
    let mut directory = filesystem
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");

    let failure = directory
        .persist(
            &Path::parse("/published-directory").expect("target should parse"),
            PersistOptions::default(),
        )
        .expect_err("non-atomic provider result must violate requirement");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(TempResourceState::CleanupRequired, directory.state());
}

/// Rejects a provider outcome that reports a different persistence target.
#[test]
fn test_temp_directory_rejects_mismatched_persist_target() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");

    let failure = directory
        .persist(
            &Path::parse("/wrong-persist-target").expect("target should parse"),
            PersistOptions::default(),
        )
        .expect_err("mismatched target should violate the provider contract");

    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(PersistFailureState::Indeterminate, failure.state());
    assert_eq!(TempResourceState::Indeterminate, directory.state());
}

/// Verifies keeping a temporary directory transfers cleanup responsibility and
/// leaves the completed handle unusable for further persistence.
#[test]
fn test_temp_directory_keep_releases_cleanup_responsibility() {
    let (filesystem, cleanup_calls, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");

    directory.keep().expect("keep should succeed");
    assert_eq!(TempResourceState::Kept, directory.state());
    assert!(
        directory
            .persist(
                &Path::parse("/published-directory")
                    .expect("target should parse"),
                PersistOptions::default(),
            )
            .is_err()
    );
    assert!(directory.cleanup().is_err());
    drop(directory);
    assert_eq!(
        0,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}

/// Verifies every provider-reported persist failure state is retained by the
/// directory facade so callers know whether cleanup remains possible.
#[test]
fn test_temp_directory_persist_failure_preserves_provider_progress() {
    for (failure_state, expected_state) in [
        (PersistFailureState::NotPublished, TempResourceState::Owned),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
        ),
        (
            PersistFailureState::Indeterminate,
            TempResourceState::Indeterminate,
        ),
    ] {
        let (filesystem, cleanup_calls, persist_calls) =
            crate::handle_support::temp_failure_filesystem(failure_state);
        let mut directory = filesystem
            .create_temp_directory(TempDirectoryOptions::default())
            .expect("temporary directory should open");

        let failure = directory
            .persist(
                &Path::parse("/published-directory")
                    .expect("target should parse"),
                PersistOptions::default(),
            )
            .expect_err("injected provider persistence failure should surface");
        assert_eq!(FsOperation::PersistTemp, failure.error().operation());
        assert_eq!(
            Some(&Path::parse("/temporary").expect("path should parse")),
            failure.error().path()
        );
        assert_eq!(
            Some(
                &Path::parse("/published-directory")
                    .expect("target should parse")
            ),
            failure.error().target()
        );
        assert_eq!(Some("handles-test"), failure.error().provider());
        assert_eq!(failure_state, failure.state());
        assert_eq!(expected_state, directory.state());
        assert_eq!(
            1,
            *persist_calls.lock().expect("persist lock should succeed")
        );
        drop(directory);
        assert_eq!(
            usize::from(matches!(
                expected_state,
                TempResourceState::Owned | TempResourceState::CleanupRequired
            )),
            *cleanup_calls.lock().expect("cleanup lock should succeed")
        );
    }
}

/// Verifies directory keep and cleanup failures preserve either recoverable
/// ownership or an indeterminate state according to the provider error.
#[test]
fn test_temp_directory_lifecycle_errors_preserve_recovery_state() {
    for (operation, error_kind, expected_state) in [
        ("keep", FsErrorKind::Io, TempResourceState::Owned),
        (
            "keep",
            FsErrorKind::Indeterminate,
            TempResourceState::Indeterminate,
        ),
        (
            "cleanup",
            FsErrorKind::Io,
            TempResourceState::CleanupRequired,
        ),
        (
            "cleanup",
            FsErrorKind::Indeterminate,
            TempResourceState::Indeterminate,
        ),
    ] {
        let (filesystem, cleanup_calls) =
            crate::handle_support::temp_lifecycle_error_filesystem(
                (operation == "keep").then_some(error_kind),
                (operation == "cleanup").then_some(error_kind),
            );
        let mut directory = filesystem
            .create_temp_directory(TempDirectoryOptions::default())
            .expect("temporary directory should open");

        let result = if operation == "keep" {
            directory.keep()
        } else {
            directory.cleanup()
        };
        let error = result
            .expect_err("lifecycle operation should surface provider error");
        assert_eq!(
            if operation == "keep" {
                FsOperation::KeepTemp
            } else {
                FsOperation::CleanupTemp
            },
            error.operation()
        );
        assert_eq!(
            Some(&Path::parse("/temporary").expect("path should parse")),
            error.path()
        );
        assert_eq!(Some("handles-test"), error.provider());
        assert_eq!(expected_state, directory.state());
        drop(directory);
        assert_eq!(
            usize::from(operation == "cleanup")
                + usize::from(matches!(
                    expected_state,
                    TempResourceState::Owned
                        | TempResourceState::CleanupRequired
                )),
            *cleanup_calls.lock().expect("cleanup lock should succeed")
        );
    }
}
