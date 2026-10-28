// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Diagnostic context coverage for copy preflight and provider outcomes.

#![cfg(feature = "async")]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use common::async_recording_spi::AsyncRecordingConfig;
use common::async_recording_spi::async_recording_file_system;
use common::poll_support::ready;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::CopyOptions;
use qubit_fs::directory::CreateDirectoryOptions;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOptions;
use qubit_fs::directory::DeleteOutcome;
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
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ReadOptions;
use qubit_fs::rename::RenameOptions;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}

struct SyncOutcomeSpi {
    calls: Arc<AtomicUsize>,
}

impl FileSystemSpi for SyncOutcomeSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-recording").expect("test id should be valid"),
                "async-recording",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::CreateDirectory)
                .with(ProviderOperation::DeleteFile)
                .with(ProviderOperation::DeleteDirectory),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::CreateDirectory)
                .with_guaranteed(FileSystemCapability::Delete),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("test properties should be valid")
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }

    fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(CreateDirectoryOutcome::new(true))
    }

    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(DeleteOutcome::new(true))
    }
}

#[test]
fn test_assess_copy_preflight_preserves_request_context() {
    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig::default());
    let sync_calls = Arc::new(AtomicUsize::new(0));
    let sync = FileSystem::from_spi(SyncOutcomeSpi {
        calls: Arc::clone(&sync_calls),
    })
    .expect("sync facade should construct");
    for (source, target, options, kind) in [
        ("/same", "/same", CopyOptions::default(), FsErrorKind::InvalidOptions),
        (
            "/source",
            "/target",
            CopyOptions::default().with_max_entries(Some(0)),
            FsErrorKind::RequirementNotMet,
        ),
    ] {
        let source = path(source);
        let target = path(target);
        let error = filesystem
            .assess_copy(&source, &target, &options)
            .expect_err("preflight should reject");
        assert_eq!(kind, error.kind());
        assert_eq!(FsOperation::Copy, error.operation());
        assert_eq!(Some("async-recording"), error.provider());
        assert_eq!(Some(&source), error.path());
        assert_eq!(Some(&target), error.target());
    }
    let same = path("/same");
    let sync_error = sync
        .assess_copy(&same, &same, &CopyOptions::default())
        .expect_err("sync preflight should reject same paths");
    assert_eq!(FsOperation::Copy, sync_error.operation());
    assert_eq!(Some("async-recording"), sync_error.provider());
    assert_eq!(Some(&same), sync_error.path());
    assert_eq!(Some(&same), sync_error.target());
    assert_eq!(
        0,
        sync_calls.load(Ordering::SeqCst),
        "local rejection must not call provider"
    );
    assert!(probe.calls().is_empty(), "local copy rejection must not call provider");

    let (filesystem, probe) = async_recording_file_system(AsyncRecordingConfig {
        omitted_capability: Some(FileSystemCapability::Copy),
        omit_read_and_write: true,
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/target");
    let error = filesystem
        .assess_copy(&source, &target, &CopyOptions::default())
        .expect_err("missing copy routes should be rejected");
    assert_eq!(FsErrorKind::RequirementNotMet, error.kind());
    assert_eq!(FsOperation::Copy, error.operation());
    assert_eq!(Some("async-recording"), error.provider());
    assert_eq!(Some(&source), error.path());
    assert_eq!(Some(&target), error.target());
    assert!(probe.calls().is_empty(), "route rejection must not call provider");

    let invalid_target = path("relative");
    let error = filesystem
        .assess_copy(&path("/source"), &invalid_target, &CopyOptions::default())
        .expect_err("invalid target should be rejected");
    assert_eq!(Some(&invalid_target), error.path());
}

#[test]
fn test_sync_async_outcome_diagnostics_match() {
    let config = AsyncRecordingConfig {
        create_directory_already_existed: true,
        delete_already_missing: true,
        ..AsyncRecordingConfig::default()
    };
    let (filesystem, _) = async_recording_file_system(config);
    let sync_calls = Arc::new(AtomicUsize::new(0));
    let sync = FileSystem::from_spi(SyncOutcomeSpi {
        calls: Arc::clone(&sync_calls),
    })
    .expect("sync facade should construct");
    let resource_path = path("/resource");
    let sync_create = sync
        .create_directory(&resource_path, CreateDirectoryOptions::default())
        .expect_err("existing directory without exists_ok should violate the contract");
    let create = ready(filesystem.create_directory(&resource_path, CreateDirectoryOptions::default()))
        .expect_err("existing directory without exists_ok should violate the contract");
    assert_diagnostics_match(&sync_create, &create);

    let sync_delete = sync
        .delete_file(&resource_path, DeleteOptions::default())
        .expect_err("missing target without missing_ok should violate the contract");
    let delete = ready(filesystem.delete_file(&resource_path, DeleteOptions::default()))
        .expect_err("missing target without missing_ok should violate the contract");
    assert_diagnostics_match(&sync_delete, &delete);
    assert_eq!(2, sync_calls.load(Ordering::SeqCst));
}

fn assert_diagnostics_match(sync: &qubit_fs::FsError, asynchronous: &qubit_fs::FsError) {
    assert_eq!(FsErrorKind::ProviderContractViolation, sync.kind());
    assert_eq!(sync.kind(), asynchronous.kind());
    assert_eq!(sync.operation(), asynchronous.operation());
    assert_eq!(Some("async-recording"), sync.provider());
    assert_eq!(sync.provider(), asynchronous.provider());
    assert_eq!(sync.path(), asynchronous.path());
    assert_eq!(sync.target(), asynchronous.target());
    assert_eq!(sync.effect_state(), asynchronous.effect_state());
    assert_eq!(
        std::error::Error::source(sync).is_some(),
        std::error::Error::source(asynchronous).is_some()
    );
    assert_eq!(None, sync.target());
    assert_eq!(None, sync.effect_state());
    assert!(std::error::Error::source(sync).is_none());
    assert!(matches!(sync.operation(), FsOperation::CreateDir | FsOperation::Delete));
}

#[test]
fn test_identity_and_rename_diagnostics_keep_their_operations() {
    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        invalid_opened_identity: true,
        ..AsyncRecordingConfig::default()
    });
    let resource_path = path("/resource");
    let error = ready(filesystem.open_reader(&resource_path, ReadOptions::default()))
        .expect_err("provider returned reader with invalid identity");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(FsOperation::ValidateProviderOutcome, error.operation());
    assert_eq!(Some(&resource_path), error.path());

    let (filesystem, _) = async_recording_file_system(AsyncRecordingConfig {
        rename_atomicity: Some(AchievedAtomicity::Atomic),
        ..AsyncRecordingConfig::default()
    });
    let source = path("/source");
    let target = path("/wrong-rename-target");
    let failure = ready(filesystem.rename(&source, &target, RenameOptions::default()))
        .expect_err("provider returned a different rename target");
    assert_eq!(FsErrorKind::ProviderContractViolation, failure.error().kind());
    assert_eq!(FsOperation::Rename, failure.error().operation());
    assert_eq!(Some(&source), failure.error().path());
    assert_eq!(Some(&target), failure.error().target());
}
