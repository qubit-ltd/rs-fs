// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Cursor;

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
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
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

/// A minimal synchronous provider that serves a single health resource.
///
/// The provider reports `/health` as a two-byte file containing `ok` and
/// rejects every other path.
///
/// # Examples
///
/// ```
/// use qubit_fs::FileSystem;
/// use qubit_fs::FsResult;
/// use qubit_fs::Path;
/// use qubit_fs::read::ReadOptions;
/// use qubit_fs_documentation_examples::provider_minimal::HealthProvider;
///
/// fn main() -> FsResult<()> {
///     let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
///     let body = filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)?;
///     assert_eq!(body, b"ok");
///     Ok(())
/// }
/// ```
pub struct HealthProvider {
    /// Properties advertised to the filesystem when this provider is mounted.
    properties: ProviderProperties,
}

impl HealthProvider {
    /// Creates a provider configured to serve `/health`.
    ///
    /// # Returns
    ///
    /// A provider with the required stat and reader operations enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the filesystem identity or provider properties are
    /// invalid.
    pub fn new() -> FsResult<Self> {
        let id = FileSystemId::new("docs-health")?;
        let properties = ProviderProperties::new(
            FileSystemInfo::new(id, "docs-health", PathSemantics::Hierarchical),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )?;
        Ok(Self { properties })
    }

    /// Rejects paths other than the single resource served by this provider.
    ///
    /// # Parameters
    ///
    /// - `path`: The requested logical path.
    /// - `operation`: The provider operation being attempted.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` for `/health`.
    ///
    /// # Errors
    ///
    /// Returns a `NotFound` error for any path other than `/health`, preserving
    /// the rejected path and requested operation in the error context.
    fn validate_path(path: &Path, operation: FsOperation) -> FsResult<()> {
        if path.as_str() == "/health" {
            Ok(())
        } else {
            Err(FsError::new(FsErrorKind::NotFound, operation, "health resource not found").with_path(path.clone()))
        }
    }
}

impl FileSystemSpi for HealthProvider {
    /// Returns a clone of the provider properties used when it was created.
    ///
    /// # Returns
    ///
    /// The immutable provider property snapshot.
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    /// Reports `/health` as a two-byte regular file.
    ///
    /// # Parameters
    ///
    /// - `request`: The validated request containing the path to inspect.
    ///
    /// # Returns
    ///
    /// Metadata identifying `/health` as a two-byte regular file.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when the request targets a path other than `/health`.
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Self::validate_path(request.path(), FsOperation::Stat)?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(2)),
        ))
    }

    /// Opens `/health` as a reader over the bytes `ok`.
    ///
    /// # Parameters
    ///
    /// - `request`: The validated request containing the path to open.
    ///
    /// # Returns
    ///
    /// A reader bound to this provider's filesystem identity and `/health`.
    ///
    /// # Errors
    ///
    /// Returns `NotFound` when the request targets a path other than `/health`.
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Self::validate_path(request.path(), FsOperation::OpenReader)?;
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"ok".to_vec())),
        ))
    }
}

/// Mounts the minimal provider and reads its `/health` resource.
///
/// # Returns
///
/// Returns the resource bytes, which are `ok` for this provider.
///
/// # Errors
///
/// Returns an error if provider construction, filesystem mounting, path
/// parsing, or reading the resource fails.
pub fn read_health() -> FsResult<Vec<u8>> {
    let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
    filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)
}
