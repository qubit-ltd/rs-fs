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

pub struct HealthProvider {
    properties: ProviderProperties,
}

impl HealthProvider {
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

    fn validate_path(path: &Path, operation: FsOperation) -> FsResult<()> {
        if path.as_str() == "/health" {
            Ok(())
        } else {
            Err(FsError::new(
                FsErrorKind::NotFound,
                operation,
                "health resource not found",
            )
            .with_path(path.clone()))
        }
    }
}

impl FileSystemSpi for HealthProvider {
    fn properties(&self) -> ProviderProperties {
        self.properties.clone()
    }

    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Self::validate_path(request.path(), FsOperation::Stat)?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(2)),
        ))
    }

    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Self::validate_path(request.path(), FsOperation::OpenReader)?;
        Ok(OpenedReader::new(
            OpenedFileInfo::new(self.properties.info().id().clone(), request.path().clone()),
            Box::new(Cursor::new(b"ok".to_vec())),
        ))
    }
}

pub fn read_health() -> FsResult<Vec<u8>> {
    let filesystem = FileSystem::from_spi(HealthProvider::new()?)?;
    filesystem.read_all(&Path::parse("/health")?, ReadOptions::default(), 16)
}
