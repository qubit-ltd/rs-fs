// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Recording-provider tests for the non-emulated rename primitive.

use qubit_fs::spi::{
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    FileSystemSpi,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedDirectoryStream,
    OpenedReader,
    OpenedTempDirectory,
    OpenedTempFile,
    OpenedWriter,
    RenameRequest,
    SpiRenameFailure,
    StatRequest,
    StatResponse,
};
use qubit_fs::{
    AchievedAtomicity,
    AtomicityRequirement,
    CreateDirectoryOutcome,
    DeleteOutcome,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    Path,
    PathConstraints,
    PublicationMethod,
    RenameFailureState,
    RenameOptions,
    RenameOutcome,
};
use std::sync::{
    Arc,
    Mutex,
};

/// Selects an atomic or downgraded provider rename result.
struct RenameSpi {
    atomicity: AchievedAtomicity,
    method: PublicationMethod,
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
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("rename-recording").expect("valid id"),
                "rename-recording",
                qubit_fs::PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with(FileSystemCapability::Rename)
                .with(FileSystemCapability::AtomicRename),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
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
        _: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        self.calls
            .lock()
            .expect("calls lock should succeed")
            .push("rename");
        Ok(RenameOutcome::new(self.atomicity, self.method))
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
            RenameOptions {
                overwrite: false,
                atomicity: AtomicityRequirement::Required,
            },
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
