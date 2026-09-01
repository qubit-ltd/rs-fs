// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::temp::PersistFailureState;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::TempOptions;
use qubit_fs::temp::TempResourceState;
#[test]
fn test_required_non_atomic_temp_persist_retains_cleanup_responsibility() {
    let (filesystem, cleanup_calls, _) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");
    assert_eq!("/temporary", temporary.path().as_str());
    assert!(format!("{temporary:?}").contains("TempFile"));
    let error = temporary
        .persist(
            &Path::parse("/target").expect("target should parse"),
            PersistOptions::default(),
        )
        .expect_err("non-atomic result must violate required contract");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.error().kind());
    assert_eq!(TempResourceState::CleanupRequired, temporary.state());
    temporary.cleanup().expect("cleanup should remain available");
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup lock should succeed"));
}

#[test]
fn test_temp_file_illegal_target_fails_preflight_without_provider_persist_and_remains_owned() {
    let (filesystem, cleanup_calls, persist_calls) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");
    let error = temporary
        .persist(
            &Path::parse("relative").expect("relative path should parse"),
            PersistOptions::default(),
        )
        .expect_err("illegal target must fail before provider persist");
    assert_eq!(FsErrorKind::InvalidPath, error.error().kind());
    assert_eq!(PersistFailureState::NotPublished, error.state());
    assert_eq!(TempResourceState::Owned, temporary.state());
    assert_eq!(0, *persist_calls.lock().expect("persist lock should succeed"));
    temporary.cleanup().expect("owned resource should remain recoverable");
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup lock should succeed"));
}

#[test]
fn test_temp_file_rejects_provider_path_outside_facade_constraints() {
    let error = crate::handle_support::invalid_temp_path_filesystem()
        .create_temp_file(TempOptions::default())
        .expect_err("relative provider temporary path must be rejected");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

/// Verifies a preferred-atomicity persist accepts a non-atomic provider result
/// and makes the completed file handle unavailable for further cleanup or keep.
#[test]
fn test_temp_file_persist_marks_resource_persisted() {
    let (filesystem, cleanup_calls, persist_calls) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");
    let target = Path::parse("/published-file").expect("target should parse");

    let outcome = temporary
        .persist(
            &target,
            PersistOptions::default().with_atomicity(AtomicityRequirement::Preferred),
        )
        .expect("preferred atomicity may accept non-atomic persistence");
    assert_eq!(&target, outcome.target());
    assert_eq!(TempResourceState::Persisted, temporary.state());
    assert_eq!(1, *persist_calls.lock().expect("persist lock should succeed"));
    assert!(temporary.cleanup().is_err());
    assert!(temporary.keep().is_err());
    drop(temporary);
    assert_eq!(0, *cleanup_calls.lock().expect("cleanup lock should succeed"));
}

/// Verifies a provider cannot claim that a temporary resource was persisted to
/// a target different from the caller's requested final path.
#[test]
fn test_temp_file_persist_rejects_wrong_provider_target() {
    let (filesystem, _, _) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");
    let requested = Path::parse("/wrong-persist-target").expect("target should parse");

    let failure = temporary
        .persist(
            &requested,
            PersistOptions::default().with_atomicity(AtomicityRequirement::Preferred),
        )
        .expect_err("wrong provider target must violate the contract");
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(PersistFailureState::Indeterminate, failure.state());
    assert_eq!(TempResourceState::Indeterminate, temporary.state());
}

/// Verifies a cleaned temporary file cannot be persisted or kept and does not
/// cause a second best-effort cleanup when dropped.
#[test]
fn test_temp_file_cleanup_marks_resource_cleaned() {
    let (filesystem, cleanup_calls, _) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");

    temporary.cleanup().expect("cleanup should succeed");
    assert_eq!(TempResourceState::Cleaned, temporary.state());
    assert!(temporary.keep().is_err());
    assert!(
        temporary
            .persist(
                &Path::parse("/published-file").expect("target should parse"),
                PersistOptions::default(),
            )
            .is_err()
    );
    drop(temporary);
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup lock should succeed"));
}

/// Verifies keeping an owned temporary file succeeds and transfers automatic
/// cleanup responsibility without publishing the source.
#[test]
fn test_temp_file_keep_marks_resource_kept() {
    let (filesystem, cleanup_calls, _) = crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(TempOptions::default())
        .expect("temporary file should open");

    temporary.keep().expect("keep should succeed");
    assert_eq!(TempResourceState::Kept, temporary.state());
    drop(temporary);
    assert_eq!(0, *cleanup_calls.lock().expect("cleanup lock should succeed"));
}

/// Verifies temporary file persistence retains each provider-confirmed
/// failure state instead of assuming that an error left the source unchanged.
#[test]
fn test_temp_file_persist_failure_preserves_provider_progress() {
    for (failure_state, expected_state) in [
        (PersistFailureState::NotPublished, TempResourceState::Owned),
        (
            PersistFailureState::PublishedSourceRetained,
            TempResourceState::CleanupRequired,
        ),
        (PersistFailureState::Indeterminate, TempResourceState::Indeterminate),
    ] {
        let (filesystem, cleanup_calls, persist_calls) = crate::handle_support::temp_failure_filesystem(failure_state);
        let mut temporary = filesystem
            .create_temp_file(TempOptions::default())
            .expect("temporary file should open");

        let failure = temporary
            .persist(
                &Path::parse("/published-file").expect("target should parse"),
                PersistOptions::default(),
            )
            .expect_err("injected provider persistence failure should surface");
        assert_eq!(FsOperation::PersistTemp, failure.error().operation());
        assert_eq!(
            Some(&Path::parse("/temporary").expect("path should parse")),
            failure.error().path()
        );
        assert_eq!(
            Some(&Path::parse("/published-file").expect("target should parse")),
            failure.error().target()
        );
        assert_eq!(Some("handles-test"), failure.error().provider());
        assert_eq!(failure_state, failure.state());
        assert_eq!(expected_state, temporary.state());
        assert_eq!(1, *persist_calls.lock().expect("persist lock should succeed"));
        drop(temporary);
        assert_eq!(
            usize::from(matches!(
                expected_state,
                TempResourceState::Owned | TempResourceState::CleanupRequired
            )),
            *cleanup_calls.lock().expect("cleanup lock should succeed")
        );
    }
}

/// Verifies file keep and cleanup errors retain the exact recovery state that
/// callers need before deciding whether to retry or abandon the source.
#[test]
fn test_temp_file_lifecycle_errors_preserve_recovery_state() {
    for (operation, error_kind, expected_state) in [
        ("keep", FsErrorKind::Io, TempResourceState::Owned),
        ("keep", FsErrorKind::Indeterminate, TempResourceState::Indeterminate),
        ("cleanup", FsErrorKind::Io, TempResourceState::CleanupRequired),
        ("cleanup", FsErrorKind::Indeterminate, TempResourceState::Indeterminate),
    ] {
        let (filesystem, cleanup_calls) = crate::handle_support::temp_lifecycle_error_filesystem(
            (operation == "keep").then_some(error_kind),
            (operation == "cleanup").then_some(error_kind),
        );
        let mut temporary = filesystem
            .create_temp_file(TempOptions::default())
            .expect("temporary file should open");

        let result = if operation == "keep" {
            temporary.keep()
        } else {
            temporary.cleanup()
        };
        let error = result.expect_err("lifecycle operation should surface provider error");
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
        assert_eq!(expected_state, temporary.state());
        drop(temporary);
        assert_eq!(
            usize::from(operation == "cleanup")
                + usize::from(matches!(
                    expected_state,
                    TempResourceState::Owned | TempResourceState::CleanupRequired
                )),
            *cleanup_calls.lock().expect("cleanup lock should succeed")
        );
    }
}
