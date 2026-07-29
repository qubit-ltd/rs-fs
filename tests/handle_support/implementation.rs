// qubit-style: allow test-file-name -- this module is included by
// handle_support/mod.rs.
use std::io::Result as IoResult;
use std::sync::{Arc, Mutex};

use qubit_fs::spi::{
    CreateDirectoryRequest, CreateTempDirectoryRequest, CreateTempFileRequest,
    DeleteDirectoryRequest, DeleteFileRequest, DirectoryStreamSpi, FileSystemSpi, FileWriterSpi,
    ListRequest, OpenReaderRequest, OpenWriterRequest, OpenedDirectoryStream, OpenedReader,
    OpenedTempDirectory, OpenedTempFile, OpenedWriter, PersistRequest, RenameRequest,
    SpiRenameFailure, SpiWriteFailure, StatRequest, StatResponse, TempResourceSpi,
};
use qubit_fs::{
    AchievedAtomicity, CreateDirectoryOutcome, DeleteOutcome, DirEntry, FileMetadata, FileSystem,
    FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo, FileSystemLimits,
    FileSystemProperties, FsError, FsErrorKind, FsOperation, FsResult, OpenedFileInfo, Path,
    PathConstraints, PersistOutcome, PublicationMethod, RenameFailureState, RenameOutcome,
    WriteFailureState, WriteOutcome,
};
use qubit_io::Output;

pub(crate) struct BehaviorSpi {
    pub(crate) fail_commit: bool,
    pub(crate) fail_write: bool,
    pub(crate) limits: FileSystemLimits,
    pub(crate) entries: Mutex<Vec<DirEntry>>,
    pub(crate) cleanup_calls: Arc<Mutex<usize>>,
    pub(crate) persist_calls: Arc<Mutex<usize>>,
    pub(crate) temp_path: Path,
}
pub(crate) fn filesystem(
    fail_commit: bool,
    entries: Vec<DirEntry>,
) -> (FileSystem, Arc<Mutex<usize>>, Arc<Mutex<usize>>) {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let spi = BehaviorSpi {
        fail_commit,
        fail_write: false,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(entries),
        cleanup_calls: Arc::clone(&cleanup_calls),
        persist_calls: Arc::clone(&persist_calls),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
    };
    (
        FileSystem::from_spi(spi).expect("facade should construct"),
        cleanup_calls,
        persist_calls,
    )
}
pub(crate) fn limited_write_filesystem(maximum: u64) -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        limits: FileSystemLimits::unknown()
            .with_max_write_bytes(qubit_fs::FileSystemLimit::Maximum(maximum)),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
    })
    .expect("facade should construct")
}
pub(crate) fn stream_failure_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: true,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
    })
    .expect("facade should construct")
}
pub(crate) fn invalid_temp_path_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("relative").expect("test path should parse"),
    })
    .expect("facade should construct")
}

#[test]
fn test_handle_support_constructs_file_system() {
    let (file_system, _, _) = filesystem(false, Vec::new());

    assert_eq!(
        "handles-test",
        file_system.properties().info().id().as_str()
    );
}

impl BehaviorSpi {
    fn unsupported() -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Other,
            "unused test operation",
        )
    }
    fn info(&self) -> OpenedFileInfo {
        OpenedFileInfo::new(
            FileSystemId::new("handles-test").expect("valid test id"),
            self.temp_path.clone(),
        )
    }
}
impl FileSystemSpi for BehaviorSpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("handles-test").expect("valid test id"),
                "handles-test",
                qubit_fs::PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with(FileSystemCapability::List)
                .with(FileSystemCapability::Write)
                .with(FileSystemCapability::AtomicReplace)
                .with(FileSystemCapability::TempFile)
                .with(FileSystemCapability::TempDirectory)
                .with(FileSystemCapability::AtomicTempPersist),
            self.limits,
            PathConstraints::absolute(),
        )
        .expect("valid test properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(qubit_fs::FileKind::File),
        ))
    }
    fn list(&self, _: ListRequest<'_>) -> FsResult<OpenedDirectoryStream> {
        Ok(OpenedDirectoryStream::new(Box::new(Entries(
            std::mem::take(&mut *self.entries.lock().expect("entries lock should succeed")),
        ))))
    }
    fn open_reader(&self, _: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        Err(Self::unsupported())
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(
                FileSystemId::new("handles-test").expect("valid test id"),
                request.path().clone(),
            ),
            Box::new(Writer {
                fail_commit: self.fail_commit,
                fail_write: self.fail_write,
            }),
        ))
    }
    fn create_directory(&self, _: CreateDirectoryRequest<'_>) -> FsResult<CreateDirectoryOutcome> {
        Err(Self::unsupported())
    }
    fn delete_file(&self, _: DeleteFileRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(Self::unsupported())
    }
    fn delete_directory(&self, _: DeleteDirectoryRequest<'_>) -> FsResult<DeleteOutcome> {
        Err(Self::unsupported())
    }
    fn rename(&self, _: RenameRequest<'_>) -> Result<RenameOutcome, SpiRenameFailure> {
        Err(SpiRenameFailure::new(
            Self::unsupported(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(&self, _: CreateTempFileRequest) -> FsResult<OpenedTempFile> {
        Ok(OpenedTempFile::new(
            self.info(),
            Box::new(Temp {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                persist_calls: Arc::clone(&self.persist_calls),
                non_atomic: true,
            }),
        ))
    }
    fn create_temp_directory(
        &self,
        _: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        Ok(OpenedTempDirectory::new(
            self.info(),
            Box::new(Temp {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                persist_calls: Arc::clone(&self.persist_calls),
                non_atomic: false,
            }),
        ))
    }
}
struct Writer {
    fail_commit: bool,
    fail_write: bool,
}
impl Output for Writer {
    type Item = u8;
    unsafe fn write_unchecked(&mut self, _: &[u8], _: usize, count: usize) -> IoResult<usize> {
        if self.fail_write {
            Err(std::io::Error::other("stream secret=top-secret"))
        } else {
            Ok(count)
        }
    }
    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}
impl FileWriterSpi for Writer {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        if self.fail_commit {
            Err(SpiWriteFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CommitWriter,
                    "injected commit failure",
                ),
                WriteFailureState::RetryableNotPublished,
            ))
        } else {
            Ok(WriteOutcome::new(
                AchievedAtomicity::Atomic,
                PublicationMethod::Direct,
            ))
        }
    }
    fn abort(&mut self) -> FsResult<()> {
        Ok(())
    }
}
struct Entries(Vec<DirEntry>);
impl DirectoryStreamSpi for Entries {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(self.0.pop())
    }
}
struct Temp {
    cleanup_calls: Arc<Mutex<usize>>,
    persist_calls: Arc<Mutex<usize>>,
    non_atomic: bool,
}
impl TempResourceSpi for Temp {
    fn persist(
        &mut self,
        request: PersistRequest<'_>,
    ) -> Result<PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
        *self
            .persist_calls
            .lock()
            .expect("persist lock should succeed") += 1;
        Ok(PersistOutcome::new(
            request.target().clone(),
            if self.non_atomic {
                AchievedAtomicity::NonAtomic
            } else {
                AchievedAtomicity::Atomic
            },
            PublicationMethod::Direct,
        ))
    }
    fn keep(&mut self) -> FsResult<()> {
        Ok(())
    }
    fn cleanup(&mut self) -> FsResult<()> {
        *self
            .cleanup_calls
            .lock()
            .expect("cleanup lock should succeed") += 1;
        Ok(())
    }
}
