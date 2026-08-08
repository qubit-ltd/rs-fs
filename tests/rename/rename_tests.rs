// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording-provider tests for the non-emulated rename primitive.

use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::AchievedAtomicity;
use qubit_fs::AtomicityRequirement;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOutcome;
use qubit_fs::DurabilityRequirement;
use qubit_fs::FileSystem;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::FileSystemLimits;
use qubit_fs::FileSystemProperties;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::PublicationMethod;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOptions;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Selects an atomic or downgraded provider rename result.
struct RenameSpi {
    atomicity: AchievedAtomicity,
    method: PublicationMethod,
    wrong_identity: bool,
    durable_capability: bool,
    durable_outcome: bool,
    calls: Arc<Mutex<Vec<&'static str>>>,
}
/// Builds a rename-capable facade and call probe.
fn filesystem(
    atomicity: AchievedAtomicity,
) -> (FileSystem, Arc<Mutex<Vec<&'static str>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RenameSpi {
        atomicity,
        method: PublicationMethod::AtomicRename,
        wrong_identity: false,
        durable_capability: false,
        durable_outcome: false,
        calls: Arc::clone(&calls),
    })
    .expect("facade should construct");
    (filesystem, calls)
}
/// Parses an absolute test path.
fn path(value: &str) -> Path {
    Path::parse(value).expect("test path should parse")
}
/// Returns the shared unused-operation error.
fn unused() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}
/// Implements a provider that can only rename.
impl FileSystemSpi for RenameSpi {
    fn properties(&self) -> FileSystemProperties {
        let mut capabilities = FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Rename)
            .with_guaranteed(FileSystemCapability::AtomicRename);
        if self.durable_capability {
            capabilities = capabilities
                .with_guaranteed(FileSystemCapability::DurableRename);
        }
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("rename-recording").expect("valid id"),
                "rename-recording",
                PathSemantics::Hierarchical,
            ),
            capabilities,
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid properties")
    }
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("stat");
        Err(unused())
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unused())
    }
    fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("open_reader");
        Err(unused())
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("open_writer");
        Err(unused())
    }
    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        Err(unused())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("delete");
        Err(unused())
    }
    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("rename");
        Ok(RenameOutcome::new(
            if self.wrong_identity {
                path("/reported-source")
            } else {
                request.source().clone()
            },
            if self.wrong_identity {
                path("/reported-target")
            } else {
                request.target().clone()
            },
            self.atomicity,
            self.method,
        )
        .with_durable(self.durable_outcome))
    }
    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(unused())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(unused())
    }
}

/// Verifies a provider durability downgrade is rejected after publication.
#[test]
fn test_rename_durability_downgrade_is_typed_contract_failure() {
    let filesystem = FileSystem::from_spi(RenameSpi {
        atomicity: AchievedAtomicity::Atomic,
        method: PublicationMethod::AtomicRename,
        wrong_identity: false,
        durable_capability: true,
        durable_outcome: false,
        calls: Arc::new(Mutex::new(Vec::new())),
    })
    .expect("facade should construct");

    let failure = filesystem
        .rename(
            &path("/source"),
            &path("/target"),
            RenameOptions::default()
                .with_durability(DurabilityRequirement::Required),
        )
        .expect_err("downgraded required durability must fail");

    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind(),
    );
    assert_eq!(RenameFailureState::Renamed, failure.state());
}
/// Verifies the facade attaches validated source and target identities to a
/// primitive rename result.
#[test]
fn test_rename_uses_single_primitive_and_binds_identity() {
    let (filesystem, calls) = filesystem(AchievedAtomicity::Atomic);
    let source = path("/source");
    let target = path("/target");
    let outcome = filesystem
        .rename(&source, &target, RenameOptions::default())
        .expect("rename should succeed");
    assert_eq!(&source, outcome.source());
    assert_eq!(&target, outcome.target());
    assert_eq!(
        ["rename"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}
/// Verifies a provider atomicity downgrade cannot be represented as an
/// unchanged failure.
#[test]
fn test_rename_atomicity_downgrade_is_typed_contract_failure_without_emulation()
{
    let (filesystem, calls) = filesystem(AchievedAtomicity::NonAtomic);
    let failure = filesystem
        .rename(
            &path("/source"),
            &path("/target"),
            RenameOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("downgraded required rename must fail");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(RenameFailureState::Renamed, failure.state());
    assert_eq!(
        ["rename"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies copy-and-delete provider output is rejected as a non-rename
/// contract violation.
#[test]
fn test_rename_rejects_copy_then_delete_provider_outcome() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RenameSpi {
        atomicity: AchievedAtomicity::Atomic,
        method: PublicationMethod::CopyThenDelete,
        wrong_identity: false,
        durable_capability: false,
        durable_outcome: false,
        calls: Arc::clone(&calls),
    })
    .expect("facade should construct");
    let failure = filesystem
        .rename(&path("/source"), &path("/target"), RenameOptions::default())
        .expect_err("copy and delete is not rename");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(RenameFailureState::Renamed, failure.state());
    assert_eq!(
        ["rename"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies provider-reported rename identities cannot be rewritten by the
/// facade into a successful result.
#[test]
fn test_rename_rejects_provider_outcome_with_wrong_identity() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let filesystem = FileSystem::from_spi(RenameSpi {
        atomicity: AchievedAtomicity::Atomic,
        method: PublicationMethod::AtomicRename,
        wrong_identity: true,
        durable_capability: false,
        durable_outcome: false,
        calls: Arc::clone(&calls),
    })
    .expect("facade should construct");

    let failure = filesystem
        .rename(&path("/source"), &path("/target"), RenameOptions::default())
        .expect_err("wrong provider identity must violate the contract");
    assert_eq!(
        FsErrorKind::ProviderContractViolation,
        failure.error().kind()
    );
    assert_eq!(RenameFailureState::Indeterminate, failure.state());
    assert_eq!(
        ["rename"],
        calls.lock().expect("calls lock should succeed").as_slice()
    );
}

/// Verifies a typed facade rename failure can be safely formatted and consumed
/// as the error-state pair needed by a caller's recovery policy.
#[test]
fn test_rename_failure_exposes_context_state_and_parts() {
    let (filesystem, _) = filesystem(AchievedAtomicity::NonAtomic);
    let failure = filesystem
        .rename(
            &path("/source"),
            &path("/target"),
            RenameOptions::default()
                .with_atomicity(AtomicityRequirement::Required),
        )
        .expect_err("non-atomic outcome must produce a typed failure");
    assert!(format!("{failure:?}").contains("RenameFailure"));
    assert!(!format!("{failure}").is_empty());
    assert_eq!(FsOperation::Rename, failure.error().operation());
    let as_error: &dyn std::error::Error = &failure;
    let source = as_error
        .source()
        .expect("Display/Error source should be available");
    assert!(
        !source.to_string().is_empty(),
        "error source should be exposed"
    );
    let (error, state) = failure.into_parts();
    assert_eq!(RenameFailureState::Renamed, state);
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}
