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
};

use qubit_fs::{
    CopyMethod,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirOptions,
    DeleteOptions,
    DirEntry,
    DirectoryStream,
    FileKind,
    FileMetadata,
    FileReader,
    FileSystem,
    FileSystemCapabilities,
    FileSystemConfig,
    FileSystemMetadata,
    FileSystemSpec,
    FileWriter,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    ListOptions,
    PersistOptions,
    ReadOptions,
    RenameOptions,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResource,
    TempResourceFactory,
    WriteOptions,
    WriteOutcome,
};
use qubit_spi::{
    ProviderAvailability,
    ProviderCreateError,
    ProviderDescriptor,
    ProviderName,
    ProviderRegistryError,
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

impl FileSystem for MockFs {
    fn metadata(&self) -> FileSystemMetadata {
        let mut metadata = FileSystemMetadata::new("mock");
        metadata.schemes.push("mock".to_owned());
        metadata.capabilities = FileSystemCapabilities {
            hierarchical_paths: true,
            directories: true,
            empty_directories: true,
            symlinks: true,
            range_read: true,
            append: true,
            random_write: false,
            atomic_rename: true,
            atomic_replace: true,
            conditional_write: true,
            server_side_copy: true,
            recursive_delete: true,
            temp_file: true,
            temp_dir: true,
            temp_persist: true,
            temp_persist_atomic: true,
            native_metadata: true,
        };
        metadata
    }

    fn path_metadata(&self, path: &FsPath) -> FsResult<FileMetadata> {
        let state = self.state.lock().expect("state lock should succeed");
        if state.dirs.contains(path.as_str()) {
            Ok(FileMetadata::new(FileKind::Directory))
        } else if state.files.contains(path.as_str()) {
            let mut metadata = FileMetadata::new(FileKind::File);
            metadata.len = Some(4);
            metadata.etag = Some("v1".to_owned());
            Ok(metadata)
        } else {
            Err(
                FsError::new(FsErrorKind::NotFound, FsOperation::Metadata, "missing")
                    .with_path(path.clone()),
            )
        }
    }

    fn exists(&self, path: &FsPath) -> FsResult<bool> {
        let state = self.state.lock().expect("state lock should succeed");
        Ok(state.files.contains(path.as_str()) || state.dirs.contains(path.as_str()))
    }

    fn list(&self, _path: &FsPath, _options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>> {
        Ok(Box::new(MockDirectoryStream {
            entries: vec![DirEntry::new(FsPath::parse("/a.txt")?, FileKind::File)],
        }))
    }

    fn open_reader(&self, _path: &FsPath, _options: &ReadOptions) -> FsResult<Box<dyn FileReader>> {
        if self
            .state
            .lock()
            .expect("state lock should succeed")
            .fail_read
        {
            return Ok(Box::new(ErrorReader));
        }
        Ok(Box::new(Cursor::new(b"data".to_vec())))
    }

    fn open_writer(&self, path: &FsPath, _options: &WriteOptions) -> FsResult<Box<dyn FileWriter>> {
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
        Ok(Box::new(MockWriter {
            state: self.state.clone(),
            path: path.clone(),
            bytes: Vec::new(),
            fail_write,
            fail_commit,
        }))
    }

    fn create_dir(&self, path: &FsPath, _options: &CreateDirOptions) -> FsResult<()> {
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

    fn delete(&self, path: &FsPath, _options: &DeleteOptions) -> FsResult<()> {
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

    fn rename(&self, from: &FsPath, to: &FsPath, _options: &RenameOptions) -> FsResult<()> {
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
        Ok(())
    }

    fn copy(&self, from: &FsPath, to: &FsPath, _options: &CopyOptions) -> FsResult<CopyOutcome> {
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
        Ok(CopyOutcome::new(stats, CopyMethod::Stream))
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

impl FileWriter for MockWriter {
    fn commit(self: Box<Self>) -> FsResult<WriteOutcome> {
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
        Ok(WriteOutcome {
            bytes_written: Some(self.bytes.len() as u64),
            etag: Some("committed".to_owned()),
            diagnostics: qubit_metadata::Metadata::new(),
        })
    }

    fn abort(self: Box<Self>) -> FsResult<()> {
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

#[derive(Debug)]
pub(crate) struct MockDirectoryStream {
    pub(crate) entries: Vec<DirEntry>,
}

impl DirectoryStream for MockDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(self.entries.pop())
    }
}

#[derive(Debug)]
pub(crate) struct FailingDirectoryStream;

impl DirectoryStream for FailingDirectoryStream {
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

impl DirectoryStream for PartiallyFailingDirectoryStream {
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
struct NativeTempFileHandle {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
}

impl TempResource for NativeTempFileHandle {
    fn fs(&self) -> Arc<dyn FileSystem> {
        self.fs.clone()
    }

    fn path(&self) -> &FsPath {
        &self.path
    }

    fn cleanup(self: Box<Self>) -> FsResult<()> {
        Ok(())
    }

    fn keep(self: Box<Self>) -> FsResult<FsPath> {
        Ok(self.path)
    }
}

impl TempFile for NativeTempFileHandle {
    fn persist(
        self: Box<Self>,
        _target: &FsPath,
        _options: &PersistOptions,
    ) -> FsResult<WriteOutcome> {
        Ok(WriteOutcome::default())
    }
}

#[derive(Debug)]
struct NativeTempDirHandle {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
}

impl TempResource for NativeTempDirHandle {
    fn fs(&self) -> Arc<dyn FileSystem> {
        self.fs.clone()
    }

    fn path(&self) -> &FsPath {
        &self.path
    }

    fn cleanup(self: Box<Self>) -> FsResult<()> {
        Ok(())
    }

    fn keep(self: Box<Self>) -> FsResult<FsPath> {
        Ok(self.path)
    }
}

impl TempDir for NativeTempDirHandle {
    fn persist(self: Box<Self>, _target: &FsPath, _options: &PersistOptions) -> FsResult<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct NativeTempResourceFactory;

static NATIVE_TEMP_RESOURCE_FACTORY: NativeTempResourceFactory = NativeTempResourceFactory;

impl TempResourceFactory for NativeTempResourceFactory {
    fn create_file(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempFileOptions,
    ) -> FsResult<Box<dyn TempFile>> {
        let path =
            self.make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        Ok(Box::new(NativeTempFileHandle { fs: owner, path }))
    }

    fn create_dir(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempDirOptions,
    ) -> FsResult<Box<dyn TempDir>> {
        let path =
            self.make_temp_path(options.parent.as_ref(), &options.prefix, &options.suffix)?;
        Ok(Box::new(NativeTempDirHandle { fs: owner, path }))
    }
}

#[derive(Debug)]
pub(crate) struct NativeTempFs;

impl FileSystem for NativeTempFs {
    fn metadata(&self) -> FileSystemMetadata {
        let mut metadata = FileSystemMetadata::new("native-temp");
        metadata.capabilities.temp_file = true;
        metadata.capabilities.temp_dir = true;
        metadata
    }

    fn temp_resource_factory(&self) -> &dyn TempResourceFactory {
        &NATIVE_TEMP_RESOURCE_FACTORY
    }

    fn path_metadata(&self, _path: &FsPath) -> FsResult<FileMetadata> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Metadata,
            "not used",
        ))
    }

    fn exists(&self, _path: &FsPath) -> FsResult<bool> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Exists,
            "not used",
        ))
    }

    fn list(&self, _path: &FsPath, _options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::List,
            "not used",
        ))
    }

    fn open_reader(&self, _path: &FsPath, _options: &ReadOptions) -> FsResult<Box<dyn FileReader>> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::OpenReader,
            "not used",
        ))
    }

    fn open_writer(
        &self,
        _path: &FsPath,
        _options: &WriteOptions,
    ) -> FsResult<Box<dyn FileWriter>> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::OpenWriter,
            "not used",
        ))
    }

    fn create_dir(&self, _path: &FsPath, _options: &CreateDirOptions) -> FsResult<()> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::CreateDir,
            "not used",
        ))
    }

    fn delete(&self, _path: &FsPath, _options: &DeleteOptions) -> FsResult<()> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Delete,
            "not used",
        ))
    }

    fn rename(&self, _from: &FsPath, _to: &FsPath, _options: &RenameOptions) -> FsResult<()> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Rename,
            "not used",
        ))
    }

    fn copy(&self, _from: &FsPath, _to: &FsPath, _options: &CopyOptions) -> FsResult<CopyOutcome> {
        Err(FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Copy,
            "not used",
        ))
    }
}

#[derive(Debug)]
pub(crate) struct MockProvider {
    pub(crate) fs: MockFs,
}

impl ServiceProvider<FileSystemSpec> for MockProvider {
    fn descriptor(&self) -> Result<ProviderDescriptor, ProviderRegistryError> {
        ProviderDescriptor::new("mock")?.with_aliases(&["mem"])
    }

    fn create_box(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<Box<dyn FileSystem>, ProviderCreateError> {
        Ok(Box::new(self.fs.clone()))
    }
}

#[derive(Debug)]
pub(crate) struct DescriptorErrorProvider {
    pub(crate) error: ProviderRegistryError,
}

impl ServiceProvider<FileSystemSpec> for DescriptorErrorProvider {
    fn descriptor(&self) -> Result<ProviderDescriptor, ProviderRegistryError> {
        Err(self.error.clone())
    }

    fn create_box(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<Box<dyn FileSystem>, ProviderCreateError> {
        Err(ProviderCreateError::failed("descriptor failed"))
    }
}

#[derive(Debug)]
pub(crate) struct FailingCreateProvider {
    pub(crate) id: &'static str,
    pub(crate) error: ProviderCreateError,
}

impl ServiceProvider<FileSystemSpec> for FailingCreateProvider {
    fn descriptor(&self) -> Result<ProviderDescriptor, ProviderRegistryError> {
        ProviderDescriptor::new(self.id)
    }

    fn availability(&self, _config: &FileSystemConfig) -> ProviderAvailability {
        match &self.error {
            ProviderCreateError::Unavailable { reason, .. } => {
                ProviderAvailability::unavailable(reason)
            }
            ProviderCreateError::Failed { .. } => ProviderAvailability::Available,
        }
    }

    fn create_box(
        &self,
        _config: &FileSystemConfig,
    ) -> Result<Box<dyn FileSystem>, ProviderCreateError> {
        Err(self.error.clone())
    }
}

pub(crate) fn provider_name(name: &str) -> ProviderName {
    ProviderName::new(name).expect("provider name should be valid")
}
