// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! External pending, failure, and rename coverage for the async facade.

#[path = "common/async_recording_spi.rs"]
mod async_recording_spi;
#[path = "common/poll_support.rs"]
mod poll_support;

use crate::async_recording_spi::{
    AsyncCopyStage,
    AsyncRecordingConfig,
    async_recording_file_system,
};
use crate::poll_support::{
    assert_pending,
    ready,
};
use qubit_fs::{
    AchievedAtomicity,
    FsErrorKind,
    Path,
    ReadOptions,
    RenameOptions,
    WriteOptions,
};

/// Parses one stable test path.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

/// Covers provider pending and failure propagation for facade I/O entry points.
#[test]
fn test_async_facade_stat_and_open_pending_and_error() {
    for stage in [
        AsyncCopyStage::Stat,
        AsyncCopyStage::OpenReader,
        AsyncCopyStage::OpenWriter,
    ] {
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            pending_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        match stage {
            AsyncCopyStage::Stat => {
                assert_pending(Box::pin(fs.stat(&path("/file"))).as_mut())
            }
            AsyncCopyStage::OpenReader => assert_pending(
                Box::pin(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                )
                .as_mut(),
            ),
            AsyncCopyStage::OpenWriter => assert_pending(
                Box::pin(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                )
                .as_mut(),
            ),
            _ => unreachable!(),
        }
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            failing_stage: Some(stage),
            ..AsyncRecordingConfig::default()
        });
        let error = match stage {
            AsyncCopyStage::Stat => ready(fs.stat(&path("/file")))
                .expect_err("provider failure expected"),
            AsyncCopyStage::OpenReader => {
                let Err(error) = ready(
                    fs.open_reader(&path("/file"), ReadOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            AsyncCopyStage::OpenWriter => {
                let Err(error) = ready(
                    fs.open_writer(&path("/file"), WriteOptions::default()),
                ) else {
                    panic!("provider failure expected");
                };
                error
            }
            _ => unreachable!(),
        };
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    }
}

/// Covers rename preflight and result identity binding through the async
/// facade.
#[test]
fn test_async_rename_preflight_and_result_identity() {
    let (fs, probe) = async_recording_file_system(AsyncRecordingConfig {
        rename_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/target");
    let outcome = ready(fs.rename(&source, &target, RenameOptions::default()))
        .expect("rename should succeed");
    assert_eq!(&source, outcome.source());
    assert_eq!(&target, outcome.target());
    assert_eq!(vec!["rename"], probe.calls());
    let failure = ready(fs.rename(&source, &source, RenameOptions::default()))
        .expect_err("same path must fail locally");
    assert_eq!(FsErrorKind::InvalidOptions, failure.error().kind());
    assert_eq!(vec!["rename"], probe.calls());
}
