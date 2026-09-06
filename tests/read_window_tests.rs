//! Regression coverage for read-all byte budgets and range windows.

use std::io::Cursor;
use std::sync::Arc;

use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::directory::CreateDirectoryOutcome;
use qubit_fs::directory::DeleteOutcome;
use qubit_fs::error::FsError;
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
use qubit_fs::rename::RenameFailureState;
use qubit_fs::rename::RenameOutcome;
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
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;

struct RangeSpi {
    metadata: bool,
    fail_open: bool,
}

impl RangeSpi {
    fn with_metadata() -> Self {
        Self {
            metadata: true,
            fail_open: false,
        }
    }

    fn without_metadata() -> Self {
        Self {
            metadata: false,
            fail_open: false,
        }
    }
}

impl FileSystemSpi for RangeSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("read-window").unwrap(),
                "read-window",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new().with(ProviderOperation::OpenReader),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Read)
                .with_guaranteed(FileSystemCapability::RangeRead),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat(&self, _: StatRequest<'_>) -> FsResult<StatResponse> {
        Err(unused())
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Err(unused())
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<qubit_fs::spi::OpenedReader> {
        if self.fail_open {
            return Err(FsError::new(FsErrorKind::Io, FsOperation::OpenReader, "open failed"));
        }
        let options = request.options().options();
        let total = b"0123456789";
        let start = options.offset().unwrap_or(0).min(total.len() as u64) as usize;
        let end = options
            .length()
            .map_or(total.len(), |n| start.saturating_add(n as usize).min(total.len()));
        let mut info = OpenedFileInfo::new(FileSystemId::new("read-window").unwrap(), request.path().clone());
        if self.metadata {
            info = info.with_metadata(FileMetadata::new(FileKind::File).with_len(Some(total.len() as u64)));
        }
        Ok(qubit_fs::spi::OpenedReader::new(
            info,
            Box::new(Cursor::new(total[start..end].to_vec())),
        ))
    }
    fn open_writer(&self, _: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Err(unused())
    }
    fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        Err(unused())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn delete_directory(&self, _: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(unused())
    }
    fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(unused(), RenameFailureState::Unchanged))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Err(unused())
    }
    fn create_temp_directory(&self, _: CreateTempDirectoryRequest) -> FsResult<OpenedTempDirectory> {
        Err(unused())
    }
}

fn unused() -> FsError {
    FsError::new(FsErrorKind::UnsupportedOperation, FsOperation::Other, "unused")
}

#[test]
fn metadata_length_is_compared_with_selected_window() {
    let fs = FileSystem::from_spi(RangeSpi::with_metadata()).unwrap();
    let path = Path::parse("/source").unwrap();
    let options = ReadOptions::default().with_offset(Some(2)).with_length(Some(3));
    assert_eq!(b"234", fs.read_all(&path, options.clone(), 3).unwrap().as_slice());
    assert_eq!(
        FsErrorKind::ResourceLimitExceeded,
        fs.read_all(&path, options, 2).unwrap_err().kind()
    );
}

#[test]
fn metadata_length_does_not_reject_empty_range_at_eof() {
    let fs = FileSystem::from_shared_spi(Arc::new(RangeSpi::with_metadata())).unwrap();
    let path = Path::parse("/source").unwrap();
    let options = ReadOptions::default().with_offset(Some(100));
    assert!(fs.read_all(&path, options, 0).unwrap().is_empty());
}

#[test]
fn read_all_accepts_unknown_metadata_but_still_enforces_stream_budget() {
    let fs = FileSystem::from_spi(RangeSpi::without_metadata()).unwrap();
    let path = Path::parse("/source").unwrap();
    let options = ReadOptions::default().with_offset(Some(2)).with_length(Some(3));
    assert_eq!(b"234", fs.read_all(&path, options.clone(), 3).unwrap().as_slice());
    assert_eq!(
        FsErrorKind::ResourceLimitExceeded,
        fs.read_all(&path, options, 2).unwrap_err().kind()
    );
}

#[test]
fn read_all_reports_open_error_before_budget_collection() {
    let fs = FileSystem::from_spi(RangeSpi {
        metadata: true,
        fail_open: true,
    })
    .unwrap();
    let error = fs
        .read_all(&Path::parse("/source").unwrap(), Default::default(), 16)
        .unwrap_err();
    assert_eq!(FsErrorKind::Io, error.kind());
}
