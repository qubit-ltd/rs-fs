// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for the synchronous filesystem facade.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use qubit_fs::AchievedAtomicity;
use qubit_fs::CopyOptions;
use qubit_fs::CreateDirectoryOptions;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOptions;
use qubit_fs::DeleteOutcome;
use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
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
use qubit_fs::ReadOptions;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOptions;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::TempDirectoryOptions;
use qubit_fs::TempFileOptions;
use qubit_fs::WriteOptions;
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

struct CountingSpi {
    properties: FileSystemProperties,
    property_calls: Arc<AtomicUsize>,
    stat_calls: Arc<AtomicUsize>,
    wrong_stat_path: bool,
    stat_error: Option<FsErrorKind>,
    direct_error: bool,
    unexpected_create: bool,
    unexpected_delete: bool,
}

impl CountingSpi {
    fn unsupported() -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Other,
            "unused test operation",
        )
    }
}

impl FileSystemSpi for CountingSpi {
    fn properties(&self) -> FileSystemProperties {
        self.property_calls.fetch_add(1, Ordering::SeqCst);
        self.properties.clone()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        self.stat_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(kind) = self.stat_error {
            return Err(FsError::new(
                kind,
                FsOperation::Stat,
                "injected stat error",
            ));
        }
        let path = if self.wrong_stat_path {
            Path::parse("/wrong").expect("test path should parse")
        } else {
            request.path().clone()
        };
        Ok(StatResponse::new(path, FileMetadata::new(FileKind::File)))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(Self::unsupported())
    }
    fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Err(Self::unsupported())
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(Self::unsupported())
    }
    fn create_directory(
        &self,
        _: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        if self.direct_error {
            Err(Self::unsupported())
        } else {
            Ok(CreateDirectoryOutcome::new(self.unexpected_create))
        }
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        if self.direct_error {
            Err(Self::unsupported())
        } else {
            Ok(DeleteOutcome::new(self.unexpected_delete))
        }
    }
    fn delete_directory(
        &self,
        _: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        if self.direct_error {
            Err(Self::unsupported())
        } else {
            Ok(DeleteOutcome::new(self.unexpected_delete))
        }
    }
    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        if self.direct_error {
            Err(SpiRenameFailure::new(
                Self::unsupported(),
                RenameFailureState::Unchanged,
            ))
        } else {
            Ok(RenameOutcome::new(
                request.source().clone(),
                request.target().clone(),
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            ))
        }
    }
    fn create_temp_file(
        &self,
        _: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        Err(Self::unsupported())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(Self::unsupported())
    }
}

#[test]
fn test_file_system_from_spi_caches_properties_snapshot() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    );
    let properties = properties.expect("properties should be valid");
    let property_calls = Arc::new(AtomicUsize::new(0));
    let stat_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::clone(&property_calls),
        stat_calls: Arc::clone(&stat_calls),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let clone = filesystem.clone();
    assert_eq!(
        filesystem.properties().info().provider_id(),
        clone.properties().info().provider_id()
    );
    assert_eq!(1, property_calls.load(Ordering::SeqCst));
    let relative = Path::parse("relative").expect("relative path should parse");
    let error = filesystem
        .stat(&relative)
        .expect_err("absolute-only filesystem should reject relative path");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(0, stat_calls.load(Ordering::SeqCst));
}

#[test]
fn test_stat_rejects_path_with_different_semantics_before_spi_call() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("object-store").expect("test id should be valid"),
            "test",
            PathSemantics::ObjectKey,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::either(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let stat_calls = Arc::new(AtomicUsize::new(0));
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::clone(&stat_calls),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let hierarchical =
        Path::parse("object").expect("hierarchical path should parse");
    let error = filesystem
        .stat(&hierarchical)
        .expect_err("different path semantics must fail before SPI");
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::Stat, error.operation());
    assert_eq!(Some(&hierarchical), error.path());
    assert_eq!(Some("test"), error.provider());
    assert_eq!(0, stat_calls.load(Ordering::SeqCst));
}

#[test]
fn test_stat_rejects_provider_response_for_a_different_path() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let filesystem = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: true,
        stat_error: None,
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let path = Path::parse("/requested").expect("path should parse");
    let error = filesystem
        .stat(&path)
        .expect_err("mismatched provider response must fail");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

/// Distinguishes a confirmed missing path from operational stat failures.
#[test]
fn test_exists_maps_not_found_only_and_contextualizes_other_errors() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let path = Path::parse("/requested").expect("path should parse");

    let missing = FileSystem::from_spi(CountingSpi {
        properties: properties.clone(),
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: Some(FsErrorKind::NotFound),
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    assert!(!missing.exists(&path).expect("not found is a false result"));

    let failed = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: Some(FsErrorKind::Io),
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let error = failed
        .exists(&path)
        .expect_err("non-not-found errors must propagate");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Exists, error.operation());
}

/// Preserves provider failures from synchronous directory mutations and rename
/// while adding the public operation and provider context.
#[test]
fn test_direct_sync_provider_failures_are_enriched() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::CreateDirectory)
            .with_guaranteed(FileSystemCapability::Delete)
            .with_guaranteed(FileSystemCapability::Rename)
            .with_guaranteed(FileSystemCapability::AtomicRename),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let file_system = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: true,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let target = Path::parse("/target").expect("path should parse");

    for error in [
        file_system
            .create_directory(&target, CreateDirectoryOptions::default())
            .expect_err("provider create failure should propagate"),
        file_system
            .delete_file(&target, DeleteOptions::default())
            .expect_err("provider delete failure should propagate"),
        file_system
            .delete_directory(&target, DeleteOptions::default())
            .expect_err("provider delete failure should propagate"),
    ] {
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some(&target), error.path());
        assert_eq!(Some("test"), error.provider());
    }
    let rename = file_system
        .rename(
            &Path::parse("/source").expect("path should parse"),
            &target,
            RenameOptions::default(),
        )
        .expect_err("provider rename failure should propagate");
    assert_eq!(FsErrorKind::UnsupportedOperation, rename.error().kind());
    assert_eq!(RenameFailureState::Unchanged, rename.state());
}

/// Returns a provider-confirmed rename outcome with facade-bound identity.
#[test]
fn test_sync_rename_returns_successful_provider_outcome() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Rename)
            .with_guaranteed(FileSystemCapability::AtomicRename),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let file_system = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let source = Path::parse("/source").expect("path should parse");
    let target = Path::parse("/target").expect("path should parse");
    let outcome = file_system
        .rename(&source, &target, RenameOptions::default())
        .expect("provider rename should succeed");
    assert_eq!(&source, outcome.source());
    assert_eq!(&target, outcome.target());
}

/// Rejects provider idempotency claims and same-path mutations before any
/// unintended public result can be exposed.
#[test]
fn test_sync_facade_rejects_unrequested_outcomes_and_same_path_mutations() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Copy)
            .with_guaranteed(FileSystemCapability::CreateDirectory)
            .with_guaranteed(FileSystemCapability::Delete)
            .with_guaranteed(FileSystemCapability::Rename)
            .with_guaranteed(FileSystemCapability::AtomicRename),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let file_system = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: false,
        unexpected_create: true,
        unexpected_delete: true,
    })
    .expect("facade should construct");
    let path = Path::parse("/target").expect("path should parse");

    for error in [
        file_system
            .create_directory(&path, CreateDirectoryOptions::default())
            .expect_err("unexpected existing directory must be rejected"),
        file_system
            .delete_file(&path, DeleteOptions::default())
            .expect_err("unexpected missing file must be rejected"),
        file_system
            .delete_directory(&path, DeleteOptions::default())
            .expect_err("unexpected missing directory must be rejected"),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    }
    let copy = file_system
        .copy(&path, &path, CopyOptions::default())
        .expect_err("copy source and target must differ");
    assert_eq!(FsErrorKind::InvalidOptions, copy.error().kind());
    let rename = file_system
        .rename(&path, &path, RenameOptions::default())
        .expect_err("rename source and target must differ");
    assert_eq!(FsErrorKind::InvalidOptions, rename.error().kind());
    assert_eq!(Some("test"), rename.error().provider());
}

/// Rejects synchronous operations whose advertised provider capabilities are
/// absent before dispatching any provider I/O.
#[test]
fn test_sync_facade_requires_operation_capabilities_before_dispatch() {
    let properties = FileSystemProperties::new(
        FileSystemInfo::new(
            FileSystemId::new("test").expect("test id should be valid"),
            "test",
            PathSemantics::Hierarchical,
        ),
        FileSystemCapabilities::new(),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .expect("properties should be valid");
    let file_system = FileSystem::from_spi(CountingSpi {
        properties,
        property_calls: Arc::new(AtomicUsize::new(0)),
        stat_calls: Arc::new(AtomicUsize::new(0)),
        wrong_stat_path: false,
        stat_error: None,
        direct_error: false,
        unexpected_create: false,
        unexpected_delete: false,
    })
    .expect("facade should construct");
    let path = Path::parse("/target").expect("path should parse");

    for error in [
        file_system
            .open_reader(&path, ReadOptions::default())
            .expect_err("reader requires advertised capability"),
        file_system
            .open_writer(&path, WriteOptions::default())
            .expect_err("writer requires advertised capability"),
        file_system
            .create_temp_file(TempFileOptions::default())
            .expect_err("temporary file requires advertised capability"),
        file_system
            .create_temp_directory(TempDirectoryOptions::default())
            .expect_err("temporary directory requires advertised capability"),
        file_system
            .create_directory(&path, CreateDirectoryOptions::default())
            .expect_err("directory creation requires advertised capability"),
    ] {
        assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());
    }
    let rename = file_system
        .rename(&path, &Path::root(), RenameOptions::default())
        .expect_err("rename requires advertised capability");
    assert_eq!(FsErrorKind::UnsupportedCapability, rename.error().kind());
}
