// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public deadline option contract tests.

use std::time::Duration;

use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::Path;

#[path = "handle_support/mod.rs"]
mod handle_support;

struct DelayReset;

impl Drop for DelayReset {
    fn drop(&mut self) {
        handle_support::set_writer_delays(Duration::ZERO, Duration::ZERO);
    }
}

/// Ensures a configured deadline survives option construction unchanged.
#[test]
fn test_copy_options_preserves_cumulative_deadline() {
    let deadline = Duration::from_secs(1);
    let options = CopyOptions::default().with_deadline(Some(deadline));

    assert_eq!(Some(deadline), options.deadline());
}

/// Ensures an absent deadline keeps copy operations unbounded by elapsed time.
#[test]
fn test_copy_options_default_has_no_deadline() {
    assert_eq!(None, CopyOptions::default().deadline());
}

#[test]
fn test_deadline_checked_after_flush_retains_unpublished_writer() {
    let _lock = handle_support::writer_delay_guard();
    let _reset = DelayReset;
    handle_support::set_writer_delays(Duration::from_millis(20), Duration::ZERO);
    let filesystem = handle_support::filesystem(false, Vec::new()).0;

    let failure = filesystem
        .copy(
            &Path::parse("/source").expect("source path should parse"),
            &Path::parse("/target").expect("target path should parse"),
            CopyOptions::file().with_deadline(Some(Duration::from_millis(1))),
        )
        .expect_err("slow flush should exceed the cumulative deadline");

    assert_eq!(CopyFailureState::Unchanged, failure.state());
    assert!(failure.has_writer());
}

#[test]
fn test_deadline_checked_after_commit_reports_published_without_writer() {
    let _lock = handle_support::writer_delay_guard();
    let _reset = DelayReset;
    handle_support::set_writer_delays(Duration::ZERO, Duration::from_millis(20));
    let filesystem = handle_support::filesystem(false, Vec::new()).0;

    let failure = filesystem
        .copy(
            &Path::parse("/source").expect("source path should parse"),
            &Path::parse("/target").expect("target path should parse"),
            CopyOptions::file().with_deadline(Some(Duration::from_millis(1))),
        )
        .expect_err("slow commit should exceed the cumulative deadline");

    assert_eq!(CopyFailureState::Published, failure.state());
    assert!(!failure.has_writer());
}

#[test]
fn test_indeterminate_commit_retains_writer_for_recovery() {
    let filesystem = handle_support::writer_lifecycle_filesystem(
        Some(qubit_fs::write::WriteFailureState::Indeterminate),
        None,
    );

    let failure = filesystem
        .copy(
            &Path::parse("/source").expect("source path should parse"),
            &Path::parse("/target").expect("target path should parse"),
            CopyOptions::file(),
        )
        .expect_err("indeterminate provider commit should fail");

    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert!(failure.has_writer());
}
