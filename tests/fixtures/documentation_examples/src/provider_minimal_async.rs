// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::AsyncFileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::spi::AsyncFileSystemSpi;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::SpiFuture;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// Provides metadata for one health resource without selecting a runtime.
pub struct AsyncHealthProvider {
    /// Immutable declaration returned at the SPI boundary.
    properties: ProviderProperties,
}

impl AsyncHealthProvider {
    /// Creates a provider that advertises only the implemented stat operation.
    ///
    /// # Errors
    /// Returns an invalid-properties error if the static declaration is invalid.
    pub fn new() -> FsResult<Self> {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("docs-async-health")?,
                "docs-async-health",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::Stat),
            FileSystemCapabilities::new(),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )?;
        Ok(Self { properties })
    }
}

impl AsyncFileSystemSpi for AsyncHealthProvider {
    /// Returns the cached declaration without provider I/O.
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    /// Returns metadata when polled, or a contextual not-found error.
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            let path = request.path();
            if path.as_str() != "/health" {
                return Err(FsError::new(
                    FsErrorKind::NotFound,
                    FsOperation::Stat,
                    "health resource not found",
                )
                .with_path(path.clone()));
            }
            Ok(StatResponse::new(
                path.clone(),
                FileMetadata::new(FileKind::File).with_len(Some(2)),
            ))
        })
    }
}

/// Queries the health resource through the public asynchronous facade.
///
/// # Errors
/// Returns a path, provider, or facade-contract error if metadata is unavailable.
pub async fn stat_health() -> FsResult<FileMetadata> {
    let filesystem = AsyncFileSystem::from_spi(AsyncHealthProvider::new()?)?;
    filesystem.stat(&Path::parse("/health")?).await
}
