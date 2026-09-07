// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy fallback must not invent certainty for an unsuccessful writer open.

use std::io::Cursor;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyConflictPolicy;
use qubit_fs::copy::CopyFailureState;
use qubit_fs::copy::CopyOptions;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// A readable provider whose destination open fails with controlled evidence.
struct OpenFailureSpi {
    kind: FsErrorKind,
    effect: Option<FsEffectState>,
}

impl FileSystemSpi for OpenFailureSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("open-failure").expect("valid id"),
                "test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Read)
                .with_guaranteed(FileSystemCapability::Write),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(5)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                self.properties().info().id().clone(),
                request.path().clone(),
            ),
            Box::new(Cursor::new(b"bytes".to_vec())),
        ))
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        let error = FsError::new(
            self.kind,
            FsOperation::OpenWriter,
            "controlled open failure",
        );
        Err(match self.effect {
            Some(effect) => error.with_effect_state(effect),
            None => error,
        })
    }
}

/// Uses explicit expected states, independently of production mapping logic.
fn failure_cases() -> [(FsErrorKind, Option<FsEffectState>, CopyFailureState); 6] {
    [
        (
            FsErrorKind::Io,
            Some(FsEffectState::Unchanged),
            CopyFailureState::Unchanged,
        ),
        (FsErrorKind::Io, None, CopyFailureState::Indeterminate),
        (
            FsErrorKind::Io,
            Some(FsEffectState::Applied),
            CopyFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Io,
            Some(FsEffectState::PartiallyApplied),
            CopyFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Io,
            Some(FsEffectState::Indeterminate),
            CopyFailureState::Indeterminate,
        ),
        (
            FsErrorKind::Indeterminate,
            Some(FsEffectState::Unchanged),
            CopyFailureState::Indeterminate,
        ),
    ]
}

#[test]
fn test_sync_copy_open_failure_requires_explicit_evidence() {
    for (kind, effect, expected) in failure_cases() {
        let fs = FileSystem::from_spi(OpenFailureSpi { kind, effect }).expect("valid provider");
        let failure = fs
            .copy(
                &Path::parse("/source").expect("source"),
                &Path::parse("/target").expect("target"),
                CopyOptions::default(),
            )
            .expect_err("open fails");
        assert_eq!(expected, failure.state(), "kind={kind:?} effect={effect:?}");
        assert_eq!(effect, failure.error().effect_state());
        assert!(!failure.has_writer());
    }
}

#[test]
fn test_sync_copy_skip_requires_explicit_unchanged_effect() {
    for effect in [
        Some(FsEffectState::Unchanged),
        None,
        Some(FsEffectState::Indeterminate),
    ] {
        let fs = FileSystem::from_spi(OpenFailureSpi {
            kind: FsErrorKind::AlreadyExists,
            effect,
        })
        .expect("valid provider");
        let result = fs.copy(
            &Path::parse("/source").expect("source"),
            &Path::parse("/target").expect("target"),
            CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
        );
        if effect == Some(FsEffectState::Unchanged) {
            assert_eq!(
                1,
                result.expect("proved unchanged may skip").stats().skipped
            );
        } else {
            assert_eq!(
                CopyFailureState::Indeterminate,
                result.expect_err("unknown effect must fail").state()
            );
        }
    }
}

#[cfg(feature = "async")]
#[test]
fn test_async_copy_open_failure_matches_sync_contract() {
    use crate::async_recording_spi::AsyncRecordingConfig;
    use crate::async_recording_spi::async_recording_file_system;
    use crate::poll_support::ready;
    for (kind, effect, expected) in failure_cases() {
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            decline_copy: true,
            writer_open_error: Some(kind),
            writer_open_effect: effect,
            ..Default::default()
        });
        let mut op = fs
            .begin_copy(
                Path::parse("/source").expect("source"),
                Path::parse("/target").expect("target"),
                CopyOptions::default(),
            )
            .expect("preflight");
        let failure = ready(op.execute()).expect_err("open fails");
        assert_eq!(expected, failure.state(), "kind={kind:?} effect={effect:?}");
        assert!(!op.has_recovery_writer());
    }
}

#[cfg(feature = "async")]
#[test]
fn test_async_copy_skip_requires_explicit_unchanged_effect() {
    use crate::async_recording_spi::AsyncRecordingConfig;
    use crate::async_recording_spi::async_recording_file_system;
    use crate::poll_support::ready;
    for effect in [
        Some(FsEffectState::Unchanged),
        None,
        Some(FsEffectState::Indeterminate),
    ] {
        let (fs, _) = async_recording_file_system(AsyncRecordingConfig {
            decline_copy: true,
            writer_open_error: Some(FsErrorKind::AlreadyExists),
            writer_open_effect: effect,
            ..Default::default()
        });
        let mut op = fs
            .begin_copy(
                Path::parse("/source").expect("source"),
                Path::parse("/target").expect("target"),
                CopyOptions::default().with_conflict(CopyConflictPolicy::Skip),
            )
            .expect("preflight");
        let result = ready(op.execute());
        if effect == Some(FsEffectState::Unchanged) {
            assert_eq!(
                1,
                result.expect("proved unchanged may skip").stats().skipped
            );
        } else {
            assert_eq!(
                CopyFailureState::Indeterminate,
                result.expect_err("unknown effect must fail").state()
            );
        }
    }
}
