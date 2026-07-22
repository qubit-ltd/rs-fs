// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
use std::sync::Arc;

use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
    map_async_provider_error,
};
use qubit_spi::error::ProviderErrorKind;

/// Verifies the complete filesystem-to-provider error classification table.
#[test]
fn test_async_provider_error_mapping_classifies_every_fs_error_kind() {
    for kind in [
        FsErrorKind::NotFound,
        FsErrorKind::AlreadyExists,
        FsErrorKind::NotDirectory,
        FsErrorKind::IsDirectory,
        FsErrorKind::PermissionDenied,
        FsErrorKind::AuthenticationFailed,
        FsErrorKind::InvalidState,
        FsErrorKind::Indeterminate,
        FsErrorKind::Cancelled,
        FsErrorKind::Conflict,
        FsErrorKind::PreconditionFailed,
        FsErrorKind::Timeout,
        FsErrorKind::Interrupted,
        FsErrorKind::QuotaExceeded,
        FsErrorKind::ResourceLimitExceeded,
        FsErrorKind::DataCorruption,
        FsErrorKind::Io,
        FsErrorKind::Other,
    ] {
        assert_mapping(kind, ProviderErrorKind::InitializationFailed);
    }
    assert_mapping(
        FsErrorKind::ProviderUnavailable,
        ProviderErrorKind::Unavailable,
    );
    for kind in [
        FsErrorKind::UnsupportedOperation,
        FsErrorKind::UnsupportedCapability,
        FsErrorKind::RequirementNotMet,
    ] {
        assert_mapping(kind, ProviderErrorKind::Unsupported);
    }
    for kind in [
        FsErrorKind::InvalidUri,
        FsErrorKind::InvalidPath,
        FsErrorKind::InvalidOptions,
    ] {
        assert_mapping(kind, ProviderErrorKind::InvalidConfiguration);
    }
}

/// Verifies one mapped kind and its retained filesystem source.
fn assert_mapping(kind: FsErrorKind, expected: ProviderErrorKind) {
    let error = map_async_provider_error(FsError::new(
        kind,
        FsOperation::Provider,
        "test provider failure",
    ));

    assert_eq!(expected, error.kind());
    let mut source = Error::source(&error);
    let mut retained_fs_error = false;
    while let Some(cause) = source {
        retained_fs_error |= cause.downcast_ref::<FsError>().is_some();
        retained_fs_error |= cause
            .downcast_ref::<Arc<dyn Error + Send + Sync>>()
            .and_then(|shared| shared.as_ref().downcast_ref::<FsError>())
            .is_some();
        source = cause.source();
    }
    assert!(
        retained_fs_error,
        "mapped provider error should retain FsError for {kind:?}",
    );
}
