// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
#[cfg(feature = "async")]
use crate::async_file_system::AsyncFileSystem;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::facade::facade_core::FacadeCore;
use crate::file_system::FileSystem;
use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemId;
use crate::metadata::FileSystemInfo;
use crate::metadata::FileSystemLimits;
use crate::metadata::OpenedFileInfo;
use crate::metadata::SymlinkPolicy;
use crate::path::Path;
use crate::path::PathConstraints;
use crate::path::PathSemantics;
#[cfg(feature = "async")]
use crate::spi::AsyncFileSystemSpi;
use crate::spi::FileSystemSpi;
use crate::spi::ProviderOperation;
use crate::spi::ProviderOperations;
use crate::spi::ProviderProperties;
#[cfg(feature = "async")]
use crate::spi::SpiFuture;
use crate::spi::StatRequest;
use crate::spi::StatResponse;
use crate::temp::PersistOptions;

struct PreflightTestSpi;

impl PreflightTestSpi {
    fn properties() -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("preflight-test").expect("filesystem id"),
                "preflight-test",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::Stat),
            FileSystemCapabilities::new(),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("provider properties")
    }
}

impl FileSystemSpi for PreflightTestSpi {
    fn properties(&self) -> ProviderProperties {
        Self::properties()
    }

    fn stat(&self, _request: StatRequest<'_>) -> FsResult<StatResponse> {
        unreachable!("preflight tests perform no provider calls")
    }
}

#[cfg(feature = "async")]
impl AsyncFileSystemSpi for PreflightTestSpi {
    fn properties(&self) -> ProviderProperties {
        Self::properties()
    }

    fn stat<'a>(&'a self, _request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async { unreachable!("preflight tests perform no provider calls") })
    }
}

#[test]
fn test_quantity_conversion_is_executed_at_runtime() {
    let path = Path::parse("/facade-core").expect("valid path");
    assert_eq!(
        FacadeCore::quantity_from_usize(7, FsOperation::Read, &path, "test").expect("value fits"),
        7,
    );
}

fn test_core() -> FacadeCore {
    FacadeCore::new(PreflightTestSpi::properties()).expect("facade core")
}

fn info(path: &str, kind: Option<FileKind>) -> OpenedFileInfo {
    let info = OpenedFileInfo::new(
        FileSystemId::new("preflight-test").expect("filesystem id"),
        Path::parse(path).expect("path"),
    );
    kind.map_or(info.clone(), |kind| info.with_metadata(FileMetadata::new(kind)))
}

#[test]
fn test_shared_opened_info_validation_checks_filesystem_and_requested_path() {
    let core = test_core();
    let requested = Path::parse("/expected").expect("path");
    assert!(core.validate_opened_info(&info("/expected", None), &requested).is_ok());

    let foreign = OpenedFileInfo::new(FileSystemId::new("foreign").expect("filesystem id"), requested.clone());
    let wrong_path = info("/other", None);
    for outcome in [&foreign, &wrong_path] {
        let error = core.validate_opened_info(outcome, &requested).expect_err("invalid identity");
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
        assert_eq!(FsOperation::ValidateProviderOutcome, error.operation());
        assert_eq!(Some(&requested), error.path());
        assert_eq!(Some("preflight-test"), error.provider());
    }
}

#[test]
fn test_shared_temp_info_validation_checks_metadata_paths_and_parent() {
    let core = test_core();
    let parent = Path::parse("/parent").expect("parent path");
    assert!(core
        .validate_temp_info(&info("/parent/item", Some(FileKind::File)), FileKind::File, Some(&parent))
        .is_ok());

    let wrong_id = OpenedFileInfo::new(
        FileSystemId::new("foreign").expect("filesystem id"),
        Path::parse("/parent/item").expect("path"),
    )
    .with_metadata(FileMetadata::new(FileKind::File));
    let cases = [
        (wrong_id, "/parent/item", "different filesystem"),
        (
            info("/parent/missing-metadata", None),
            "/parent/missing-metadata",
            "inconsistent resource kind",
        ),
        (
            info("/parent/wrong-kind", Some(FileKind::Directory)),
            "/parent/wrong-kind",
            "inconsistent resource kind",
        ),
        (info("relative", Some(FileKind::File)), "relative", "invalid logical path"),
        (
            info("/elsewhere/item", Some(FileKind::File)),
            "/parent",
            "outside the requested parent",
        ),
    ];
    for (outcome, expected_path, message) in cases {
        let expected_path = Path::parse(expected_path).expect("expected error path");
        let error = core
            .validate_temp_info(&outcome, FileKind::File, Some(&parent))
            .expect_err("invalid temporary resource outcome");
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
        assert_eq!(FsOperation::ValidateProviderOutcome, error.operation());
        assert_eq!(Some(&expected_path), error.path());
        assert_eq!(Some("preflight-test"), error.provider());
        assert!(error.to_string().contains(message));
    }
}

#[test]
fn test_sync_preflight_temp_persist_source_validation_context() {
    let filesystem = FileSystem::from_spi(PreflightTestSpi).expect("filesystem");
    let source = Path::parse("relative-source").expect("relative source");
    let target = Path::parse("/target").expect("target");
    let original_message = filesystem
        .properties()
        .validate_path(&source, FsOperation::PersistTemp)
        .expect_err("relative source validation")
        .to_string();
    let error = filesystem
        .preflight_temp_persist(&source, &target, &PersistOptions::default())
        .expect_err("relative source must be rejected");
    assert_eq!(Some(&source), error.path());
    assert_eq!(Some(&target), error.target());
    assert_eq!(Some(&source), error.failure_path());
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::PersistTemp, error.operation());
    assert_eq!(Some("preflight-test"), error.provider());
    assert_eq!(original_message, error.to_string());
}

#[cfg(feature = "async")]
#[test]
fn test_async_preflight_temp_persist_source_validation_context() {
    let filesystem = AsyncFileSystem::from_spi(PreflightTestSpi).expect("filesystem");
    let source = Path::parse("relative-source").expect("relative source");
    let target = Path::parse("/target").expect("target");
    let original_message = filesystem
        .properties()
        .validate_path(&source, FsOperation::PersistTemp)
        .expect_err("relative source validation")
        .to_string();
    let error = filesystem
        .preflight_temp_persist(&source, &target, &PersistOptions::default())
        .expect_err("relative source must be rejected");
    assert_eq!(Some(&source), error.path());
    assert_eq!(Some(&target), error.target());
    assert_eq!(Some(&source), error.failure_path());
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::PersistTemp, error.operation());
    assert_eq!(Some("preflight-test"), error.provider());
    assert_eq!(original_message, error.to_string());
}
