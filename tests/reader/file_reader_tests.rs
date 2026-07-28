use std::io::Cursor;
use std::sync::Arc;

use qubit_fs::spi::{
    CreateDirectoryRequest, CreateTempDirectoryRequest, CreateTempFileRequest,
    DeleteDirectoryRequest, DeleteFileRequest, FileSystemSpi, ListRequest, OpenReaderRequest,
    OpenWriterRequest, OpenedDirectoryStream, OpenedReader, OpenedTempDirectory, OpenedTempFile,
    OpenedWriter, RenameRequest, SpiRenameFailure, StatRequest, StatResponse,
};
use qubit_fs::{
    CreateDirectoryOutcome, DeleteOutcome, FileMetadata, FileSystem, FileSystemCapabilities,
    FileSystemId, FileSystemInfo, FileSystemLimits, FileSystemProperties, FsError, FsErrorKind,
    FsOperation, FsResult, OpenedFileInfo, Path, PathConstraints, RenameFailureState,
    RenameOutcome,
};

struct ReaderSpi;

impl FileSystemSpi for ReaderSpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("reader-test").expect("valid id"),
                "reader-test",
                qubit_fs::PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new().with(qubit_fs::FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
        )
        .expect("valid properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(qubit_fs::FileKind::File),
        ))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unsupported())
    }
    fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        let different = Path::parse("/different").expect("valid path");
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                FileSystemId::new("reader-test").expect("valid id"),
                different,
            ),
            Box::new(Cursor::new(b"bytes".to_vec())),
        ))
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(unsupported())
    }
    fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        Err(unsupported())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unsupported())
    }
    fn delete_directory(&self, _: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unsupported())
    }
    fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            unsupported(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Err(unsupported())
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Err(unsupported())
    }
}

fn unsupported() -> FsError {
    FsError::new(
        FsErrorKind::UnsupportedOperation,
        FsOperation::Other,
        "unused",
    )
}

#[test]
fn test_open_reader_rejects_wrong_opened_identity() {
    let file_system = FileSystem::from_shared_spi(Arc::new(ReaderSpi)).expect("facade should open");
    let requested = Path::parse("/requested").expect("valid path");
    let error = file_system
        .open_reader(&requested, Default::default())
        .expect_err("wrong identity must be rejected");
    assert_eq!(error.kind(), FsErrorKind::ProviderContractViolation);
}
