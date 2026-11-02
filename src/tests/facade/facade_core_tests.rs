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
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemId;
use crate::metadata::FileSystemInfo;
use crate::metadata::FileSystemLimits;
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
