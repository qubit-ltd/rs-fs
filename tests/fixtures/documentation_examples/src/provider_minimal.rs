use std::io::Cursor;

use qubit_fs::error::{FsErrorKind, FsOperation};
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimits, OpenedFileInfo, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::{
    FileSystemSpi, OpenReaderRequest, OpenedReader, ProviderOperation, ProviderOperations,
    ProviderProperties, StatRequest, StatResponse,
};
use qubit_fs::{FileSystem, FsError, FsResult, Path};

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
