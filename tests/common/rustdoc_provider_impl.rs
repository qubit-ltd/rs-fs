use std::collections::BTreeMap;
use std::io::Cursor;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_io::Output;

use crate::FileSystem;
use crate::Path;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::AchievedAtomicity;
use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemId;
use crate::metadata::FileSystemInfo;
use crate::metadata::FileSystemLimits;
use crate::metadata::OpenedFileInfo;
use crate::metadata::PublicationMethod;
use crate::metadata::SymlinkPolicy;
use crate::metadata::WriteOutcome;
use crate::path::PathConstraints;
use crate::path::PathSemantics;
use crate::spi::CreateTempDirectoryRequest;
use crate::spi::CreateTempFileRequest;
use crate::spi::OpenedTempDirectory;
use crate::spi::FileSystemSpi;
use crate::spi::FileWriterSpi;
use crate::spi::OpenReaderRequest;
use crate::spi::OpenWriterRequest;
use crate::spi::OpenedReader;
use crate::spi::OpenedTempFile;
use crate::spi::OpenedWriter;
use crate::spi::PersistRequest;
use crate::spi::ProviderOperation;
use crate::spi::ProviderOperations;
use crate::spi::ProviderProperties;
use crate::spi::SpiPersistFailure;
use crate::spi::SpiWriteFailure;
use crate::spi::StatRequest;
use crate::spi::StatResponse;
use crate::spi::TempResourceSpi;
use crate::temp::PersistFailureState;
use crate::temp::PersistOutcome;
use crate::write::WriteAbortOutcome;
use crate::write::WriteDisposition;
use crate::write::WriteFailureState;

type Files = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

/// Creates isolated storage containing `/report` with six bytes.
pub fn filesystem() -> FileSystem {
    let files = BTreeMap::from([("/report".to_owned(), b"report".to_vec())]);
    FileSystem::from_spi(Memory {
        files: Arc::new(Mutex::new(files)),
    })
    .unwrap()
}

struct Memory {
    files: Files,
}
impl Memory {
    fn info(&self, path: Path) -> OpenedFileInfo {
        OpenedFileInfo::new(FileSystemId::new("rustdoc").unwrap(), path)
            .with_metadata(FileMetadata::new(FileKind::File))
    }
}
impl FileSystemSpi for Memory {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("rustdoc").unwrap(),
                "rustdoc",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::Stat)
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenWriter)
                .with(ProviderOperation::CreateTempFile)
                .with(ProviderOperation::CreateTempDirectory),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::Read)
                .with_guaranteed(FileSystemCapability::Write)
                .with_guaranteed(FileSystemCapability::TempFile)
                .with_guaranteed(FileSystemCapability::TempDirectory),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        let files = self.files.lock().unwrap();
        let bytes = files
            .get(request.path().as_str())
            .ok_or_else(|| missing(FsOperation::Stat))?;
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(bytes.len() as u64)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        let files = self.files.lock().unwrap();
        let bytes = files
            .get(request.path().as_str())
            .ok_or_else(|| missing(FsOperation::OpenReader))?
            .clone();
        Ok(OpenedReader::new(
            self.info(request.path().clone()),
            Box::new(Cursor::new(bytes)),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            self.info(request.path().clone()),
            Box::new(Writer {
                files: Arc::clone(&self.files),
                path: request.path().as_str().to_owned(),
                bytes: Vec::new(),
                create_new: request.options().options().disposition() == WriteDisposition::CreateNew,
            }),
        ))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        let mut files = self.files.lock().unwrap();
        let path = Path::parse(&format!("/scratch-{}", files.len())).unwrap();
        files.insert(path.as_str().to_owned(), Vec::new());
        Ok(OpenedTempFile::new(
            self.info(path.clone()),
            Box::new(Temporary {
                files: Arc::clone(&self.files),
                source: path,
            }),
        ))
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        let mut files = self.files.lock().unwrap();
        let path = Path::parse(&format!("/scratch-dir-{}", files.len())).unwrap();
        files.insert(path.as_str().to_owned(), Vec::new());
        Ok(OpenedTempDirectory::new(
            self.info(path.clone())
                .with_metadata(FileMetadata::new(FileKind::Directory)),
            Box::new(Temporary {
                files: Arc::clone(&self.files),
                source: path,
            }),
        ))
    }
}
fn missing(operation: FsOperation) -> FsError {
    FsError::new(FsErrorKind::NotFound, operation, "example resource missing")
}
struct Writer {
    files: Files,
    path: String,
    bytes: Vec<u8>,
    create_new: bool,
}
impl Output for Writer {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, bytes: &[u8], index: usize, count: usize) -> IoResult<usize> {
        self.bytes.extend_from_slice(&bytes[index..index + count]);
        Ok(count)
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}
impl FileWriterSpi for Writer {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        let mut files = self.files.lock().unwrap();
        if self.create_new && files.contains_key(&self.path) {
            return Err(SpiWriteFailure::new(
                FsError::new(
                    FsErrorKind::AlreadyExists,
                    FsOperation::CommitWriter,
                    "example target exists",
                ),
                WriteFailureState::NotPublished,
            ));
        }
        files.insert(self.path.clone(), self.bytes.clone());
        Ok(
            WriteOutcome::new(AchievedAtomicity::NonAtomic, PublicationMethod::Direct)
                .with_bytes_written(self.bytes.len() as u64),
        )
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::NotPublished)
    }
}
struct Temporary {
    files: Files,
    source: Path,
}
impl Temporary {
    fn publish(&mut self, target: Path) -> Result<PersistOutcome, SpiPersistFailure> {
        let mut files = self.files.lock().unwrap();
        let bytes = files.remove(self.source.as_str()).ok_or_else(|| {
            SpiPersistFailure::new(
                missing(FsOperation::PersistTemp),
                PersistFailureState::NotPublishedSourceReleased,
            )
        })?;
        files.insert(target.as_str().to_owned(), bytes);
        Ok(PersistOutcome::new(
            target,
            AchievedAtomicity::NonAtomic,
            PublicationMethod::CopyThenDelete,
        ))
    }
}
impl TempResourceSpi for Temporary {
    fn persist(&mut self, request: PersistRequest<'_>) -> Result<PersistOutcome, SpiPersistFailure> {
        self.publish(request.target().clone())
    }
    fn keep(&mut self) -> Result<PersistOutcome, SpiPersistFailure> {
        self.publish(Path::parse("/kept").unwrap())
    }
    fn cleanup(&mut self) -> FsResult<()> {
        self.files.lock().unwrap().remove(self.source.as_str());
        Ok(())
    }
}
