// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

mod handle_support;

use qubit_fs::Path;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::temp::PersistFailure;
use qubit_fs::temp::PersistFailureState;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::TempOptions;
use qubit_fs::temp::TempResourceState;
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
    let (filesystem, _, _) = handle_support::temp_failure_filesystem(PersistFailureState::NotPublished);
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
        let (filesystem, cleanup_calls, persist_calls) = handle_support::temp_failure_filesystem(state);
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
        let (filesystem, cleanup_calls, persist_calls) = handle_support::temp_failure_filesystem(state);
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
        let (filesystem, _, _) = handle_support::temp_failure_filesystem(state);
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
        let (filesystem, _, _) = handle_support::temp_failure_filesystem(state);
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
        let (filesystem, calls) = handle_support::temp_lifecycle_error_filesystem(None, Some(kind));
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
    let (filesystem, cleanup_calls, persist_calls) = handle_support::temp_failure_with_cleanup_error_filesystem(
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
    let (filesystem, cleanup_calls, persist_calls) = handle_support::temp_failure_with_cleanup_error_filesystem(
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
