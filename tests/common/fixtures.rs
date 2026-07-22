// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::HashSet;
use std::fmt::Debug;
use std::io::{
    Cursor,
    Read,
    Result as IoResult,
    Write,
};
use std::sync::{
    Arc,
    Mutex,
    OnceLock,
};

use qubit_fs::{
    AchievedAtomicity,
    CopyMethod,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirOptions,
    DeleteOptions,
    DirEntry,
    DirectoryStream,
    DirectoryStreamSession,
    FileKind,
    FileLocation,
    FileMetadata,
    FileReader,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemConfig,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FileSystemResolution,
    FileSystemSpec,
    FileWriteSession,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    ListOptions,
    OpenedFileInfo,
    PathSemantics,
    PublicationMethod,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    ResourceVersion,
    WriteOptions,
    WriteOutcome,
};
use qubit_spi::error::ProviderError;
use qubit_spi::{
    ProviderDescriptor,
    ProviderId,
    ProviderMetadata,
    ServiceProvider,
};

#[derive(Debug, Default)]
pub(crate) struct MockState {
    pub(crate) files: HashSet<String>,
    pub(crate) dirs: HashSet<String>,
    pub(crate) writes: Vec<String>,
    pub(crate) deletes: Vec<String>,
    pub(crate) renames: Vec<(String, String)>,
    pub(crate) copies: Vec<(String, String)>,
    pub(crate) aborts: usize,
    pub(crate) fail_read: bool,
    pub(crate) fail_write: bool,
    pub(crate) fail_commit: bool,
    pub(crate) fail_create_dir: bool,
    pub(crate) fail_rename_unsupported: bool,
    pub(crate) fail_delete: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MockFs {
    state: Arc<Mutex<MockState>>,
}

impl MockFs {
    pub(crate) fn with_state(state: Arc<Mutex<MockState>>) -> Self {
        Self { state }
    }
}

impl FileSystemProperties for MockFs {
    fn info(&self) -> &FileSystemInfo {
        static INFO: OnceLock<FileSystemInfo> = OnceLock::new();
        INFO.get_or_init(|| {
            FileSystemInfo::new(
                FileSystemId::new("mock-instance")
                    .expect("mock filesystem id should be valid"),
                ProviderId::new("mock")
                    .expect("mock provider id should be valid"),
                PathSemantics::Hierarchical,
            )
            .with_scheme("mock")
            .expect("mock scheme should be valid")
        })
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
            .with(FileSystemCapability::List)
            .with(FileSystemCapability::Read)
            .with(FileSystemCapability::RangeRead)
            .with(FileSystemCapability::ConditionalRead)
            .with(FileSystemCapability::ChecksumValidation)
            .with(FileSystemCapability::Write)
            .with(FileSystemCapability::Append)
            .with(FileSystemCapability::ConditionalWrite)
            .with(FileSystemCapability::CreateDirectory)
            .with(FileSystemCapability::EmptyDirectory)
            .with(FileSystemCapability::Delete)
            .with(FileSystemCapability::RecursiveDelete)
            .with(FileSystemCapability::ConditionalDelete)
            .with(FileSystemCapability::Rename)
            .with(FileSystemCapability::AtomicRename)
            .with(FileSystemCapability::AtomicReplace)
            .with(FileSystemCapability::Copy)
            .with(FileSystemCapability::ServerSideCopy)
            .with(FileSystemCapability::Symlink)
            .with(FileSystemCapability::TempFile)
            .with(FileSystemCapability::TempDirectory)
            .with(FileSystemCapability::AtomicTempPersist)
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits =
            qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for MockFs {
    fn stat(&self, path: &FsPath) -> FsResult<FileMetadata> {
        let state = self.state.lock().expect("state lock should succeed");
        if state.dirs.contains(path.as_str()) {
            Ok(FileMetadata::new(FileKind::Directory))
        } else if state.files.contains(path.as_str()) {
            let mut metadata = FileMetadata::new(FileKind::File);
            metadata.len = Some(4);
            metadata.etag = Some("v1".to_owned());
            Ok(metadata)
        } else {
            Err(FsError::new(
                FsErrorKind::NotFound,
                FsOperation::Stat,
                "missing",
            )
            .with_path(path.clone()))
        }
    }

    fn exists(&self, path: &FsPath) -> FsResult<bool> {
        let state = self.state.lock().expect("state lock should succeed");
        Ok(state.files.contains(path.as_str())
            || state.dirs.contains(path.as_str()))
    }

    fn list(
        &self,
        _path: &FsPath,
        _options: ListOptions,
    ) -> FsResult<DirectoryStream> {
        Ok(DirectoryStream::new(MockDirectoryStream {
            entries: vec![DirEntry::new(
                FsPath::parse("/a.txt")?,
                FileKind::File,
            )],
        }))
    }

    fn open_reader(
        &self,
        path: &FsPath,
        _options: ReadOptions,
    ) -> FsResult<FileReader> {
        let info = mock_opened_info(path);
        if self
            .state
            .lock()
            .expect("state lock should succeed")
            .fail_read
        {
            return Ok(FileReader::new(ErrorReader, info));
        }
        Ok(FileReader::new(Cursor::new(b"data".to_vec()), info))
    }

    fn open_writer(
        &self,
        path: &FsPath,
        _options: WriteOptions,
    ) -> FsResult<FileWriter> {
        let fail_write = self
            .state
            .lock()
            .expect("state lock should succeed")
            .fail_write;
        let fail_commit = self
            .state
            .lock()
            .expect("state lock should succeed")
            .fail_commit;
        Ok(FileWriter::new(
            MockWriter {
                state: self.state.clone(),
                path: path.clone(),
                bytes: Vec::new(),
                fail_write,
                fail_commit,
            },
            mock_opened_info(path),
        ))
    }

    fn create_dir(
        &self,
        path: &FsPath,
        _options: CreateDirOptions,
    ) -> FsResult<()> {
        let mut state = self.state.lock().expect("state lock should succeed");
        if state.fail_create_dir {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::CreateDir,
                "create dir failed",
            ));
        }
        state.dirs.insert(path.as_str().to_owned());
        Ok(())
    }

    fn delete(&self, path: &FsPath, _options: DeleteOptions) -> FsResult<()> {
        let mut state = self.state.lock().expect("state lock should succeed");
        state.deletes.push(path.as_str().to_owned());
        if state.fail_delete {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::Delete,
                "delete failed",
            ));
        }
        state.files.remove(path.as_str());
        state.dirs.remove(path.as_str());
        Ok(())
    }

    fn rename(
        &self,
        from: &FsPath,
        to: &FsPath,
        _options: RenameOptions,
    ) -> FsResult<RenameOutcome> {
        let mut state = self.state.lock().expect("state lock should succeed");
        if state.fail_rename_unsupported {
            return Err(FsError::new(
                FsErrorKind::UnsupportedOperation,
                FsOperation::Rename,
                "rename unsupported",
            ));
        }
        state
            .renames
            .push((from.as_str().to_owned(), to.as_str().to_owned()));
        if state.files.remove(from.as_str()) {
            state.files.insert(to.as_str().to_owned());
        }
        if state.dirs.remove(from.as_str()) {
            state.dirs.insert(to.as_str().to_owned());
        }
        Ok(RenameOutcome::new(
            AchievedAtomicity::Atomic,
            PublicationMethod::AtomicRename,
        ))
    }

    fn copy(
        &self,
        from: &FsPath,
        to: &FsPath,
        _options: CopyOptions,
    ) -> FsResult<CopyOutcome> {
        let mut state = self.state.lock().expect("state lock should succeed");
        state
            .copies
            .push((from.as_str().to_owned(), to.as_str().to_owned()));
        state.files.insert(to.as_str().to_owned());
        let stats = CopyStats {
            files: 1,
            bytes: 4,
            ..Default::default()
        };
        Ok(CopyOutcome::new(
            stats,
            CopyMethod::Stream,
            AchievedAtomicity::NonAtomic,
        ))
    }
}

#[derive(Debug)]
struct MockWriter {
    state: Arc<Mutex<MockState>>,
    path: FsPath,
    bytes: Vec<u8>,
    fail_write: bool,
    fail_commit: bool,
}

impl Write for MockWriter {
    fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
        if self.fail_write {
            return Err(std::io::Error::other("write failed"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl FileWriteSession for MockWriter {
    fn commit(&mut self) -> FsResult<WriteOutcome> {
        if self.fail_commit {
            return Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::OpenWriter,
                "commit failed",
            ));
        }
        let mut state = self.state.lock().expect("state lock should succeed");
        state.files.insert(self.path.as_str().to_owned());
        state.writes.push(self.path.as_str().to_owned());
        let mut outcome = WriteOutcome::new(
            AchievedAtomicity::Atomic,
            PublicationMethod::Direct,
        );
        outcome.bytes_written = Some(self.bytes.len() as u64);
        outcome.version = Some(ResourceVersion::new("committed"));
        Ok(outcome)
    }

    fn abort(&mut self) -> FsResult<()> {
        self.state.lock().expect("state lock should succeed").aborts += 1;
        Ok(())
    }
}

#[derive(Debug)]
struct ErrorReader;

impl Read for ErrorReader {
    fn read(&mut self, _buffer: &mut [u8]) -> IoResult<usize> {
        Err(std::io::Error::other("read failed"))
    }
}

fn mock_opened_info(path: &FsPath) -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("mock-instance").expect("mock id should be valid"),
        path.clone(),
    ))
}

#[derive(Debug)]
pub(crate) struct MockDirectoryStream {
    pub(crate) entries: Vec<DirEntry>,
}

impl DirectoryStreamSession for MockDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(self.entries.pop())
    }
}

#[derive(Debug)]
pub(crate) struct FailingDirectoryStream;

impl DirectoryStreamSession for FailingDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Err(FsError::new(
            FsErrorKind::Io,
            FsOperation::List,
            "list failed",
        ))
    }
}

#[derive(Debug)]
pub(crate) struct PartiallyFailingDirectoryStream {
    pub(crate) entry: Option<DirEntry>,
}

impl DirectoryStreamSession for PartiallyFailingDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        if let Some(entry) = self.entry.take() {
            Ok(Some(entry))
        } else {
            Err(FsError::new(
                FsErrorKind::Io,
                FsOperation::List,
                "list failed",
            ))
        }
    }
}

#[derive(Debug)]
pub(crate) struct MockProvider {
    pub(crate) descriptor: ProviderDescriptor,
    pub(crate) fs: MockFs,
}

impl ServiceProvider<FileSystemSpec> for MockProvider {
    fn create_configured(
        &self,
        config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderError> {
        let fs: Arc<dyn FileSystem> = Arc::new(self.fs.clone());
        let path = FsPath::parse_literal(config.uri().path().as_encoded())
            .expect("mock URI path should be valid");
        Ok(FileSystemResolution::new(fs, path, config.uri().clone()))
    }
}

impl ProviderMetadata for MockProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}

#[derive(Debug)]
pub(crate) struct FailingCreateProvider {
    pub(crate) descriptor: ProviderDescriptor,
    pub(crate) error: ProviderError,
}

impl ServiceProvider<FileSystemSpec> for FailingCreateProvider {
    fn create_configured(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<FileSystemResolution<dyn FileSystem>, ProviderError> {
        Err(self.error.clone())
    }
}

impl ProviderMetadata for FailingCreateProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }
}
