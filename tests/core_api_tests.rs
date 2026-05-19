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
    AtomicityRequirement,
    Checksum,
    ChecksumAlgorithm,
    ChecksumPolicy,
    CopyConflictPolicy,
    CopyMethod,
    CopyMode,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirOptions,
    CredentialRef,
    DeleteOptions,
    DirEntry,
    DirectoryStream,
    DirectoryStreamExt,
    FileMetadata,
    FileReader,
    FileSystem,
    FileSystemCapabilities,
    FileSystemConfig,
    FileSystemExt,
    FileSystemMetadata,
    FileSystemRegistry,
    FileSystemResolver,
    FileSystemSpec,
    FileType,
    FsAuthority,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    FsUri,
    ListOptions,
    ManagedTempDir,
    ManagedTempFile,
    MetadataPreservePolicy,
    PathSemantics,
    PersistOptions,
    ProgressPolicy,
    ReadOptions,
    RenameOptions,
    ResolvedPath,
    ServerSidePreference,
    TempDir,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResources,
    WriteMode,
    WriteOptions,
    WriteOutcome,
};
use qubit_spi::{
    ProviderAvailability,
    ProviderCreateError,
    ProviderDescriptor,
    ProviderFailure,
    ProviderName,
    ProviderRegistryError,
    ServiceProvider,
};

#[derive(Debug, Default)]
struct MockState {
    files: HashSet<String>,
    dirs: HashSet<String>,
    writes: Vec<String>,
    deletes: Vec<String>,
    renames: Vec<(String, String)>,
    copies: Vec<(String, String)>,
    aborts: usize,
    fail_read: bool,
    fail_write: bool,
    fail_commit: bool,
    fail_create_dir: bool,
    fail_rename_unsupported: bool,
    fail_delete: bool,
}

#[derive(Debug, Clone, Default)]
struct MockFs {
    state: Arc<Mutex<MockState>>,
}

impl MockFs {
    fn with_state(state: Arc<Mutex<MockState>>) -> Self {
        Self { state }
    }
}

impl FileSystem for MockFs {
    fn filesystem_metadata(&self) -> FileSystemMetadata {
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

    fn metadata(&self, path: &FsPath) -> FsResult<FileMetadata> {
        let state = self.state.lock().expect("state lock should succeed");
        if state.dirs.contains(path.as_str()) {
            Ok(FileMetadata::new(FileType::Directory))
        } else if state.files.contains(path.as_str()) {
            let mut metadata = FileMetadata::new(FileType::File);
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
            entries: vec![DirEntry::new(FsPath::parse("/a.txt")?, FileType::File)],
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

    fn open_writer(
        &self,
        path: &FsPath,
        _options: &WriteOptions,
    ) -> FsResult<Box<dyn qubit_fs::FileWriter>> {
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

impl qubit_fs::FileWriter for MockWriter {
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
struct MockDirectoryStream {
    entries: Vec<DirEntry>,
}

impl DirectoryStream for MockDirectoryStream {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>> {
        Ok(self.entries.pop())
    }
}

#[derive(Debug)]
struct FailingDirectoryStream;

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
struct PartiallyFailingDirectoryStream {
    entry: Option<DirEntry>,
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
struct MockProvider {
    fs: MockFs,
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
struct DescriptorErrorProvider {
    error: ProviderRegistryError,
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
struct FailingCreateProvider {
    id: &'static str,
    error: ProviderCreateError,
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

fn provider_name(name: &str) -> ProviderName {
    ProviderName::new(name).expect("provider name should be valid")
}

#[test]
fn test_path_and_uri_models_cover_core_branches() {
    let root = FsPath::root();
    assert!(root.is_absolute());
    assert_eq!("/", root.as_str());
    assert_eq!(None, root.parent());
    assert_eq!(None, root.file_name());
    assert_eq!("/", root.to_string());

    let path = FsPath::parse("/a//b/./c").expect("path should parse");
    assert_eq!("/a/b/c", path.as_str());
    assert_eq!(Some("c"), path.file_name());
    assert_eq!(
        "/a/b",
        path.parent().expect("path should have parent").as_str()
    );
    assert_eq!(
        "/a/b/d",
        path.parent()
            .expect("parent should exist")
            .join("d")
            .expect("join should succeed")
            .as_str(),
    );
    assert_eq!(
        "/absolute",
        path.join("/absolute")
            .expect("absolute child should replace base")
            .as_str(),
    );
    assert!(FsPath::parse("").is_err());
    assert!(FsPath::parse("../escape").is_err());
    assert!(FsPath::parse("bad\0path").is_err());
    assert_eq!(
        "/child",
        root.join("child")
            .expect("root join should succeed")
            .as_str()
    );
    assert_eq!(
        "/",
        FsPath::parse("/a/..")
            .expect("absolute parent normalization should reach root")
            .as_str()
    );
    assert_eq!(
        "b",
        FsPath::parse("a/../b")
            .expect("relative parent normalization should succeed")
            .as_str()
    );
    assert!(FsPath::parse("a/..").is_err());
    assert_eq!(
        "/",
        FsPath::parse("/a")
            .expect("path should parse")
            .parent()
            .expect("absolute child should have root parent")
            .as_str()
    );
    assert_eq!(
        "a",
        FsPath::parse("a/b")
            .expect("relative path should parse")
            .parent()
            .expect("relative nested path should have parent")
            .as_str()
    );
    assert_eq!(
        None,
        FsPath::parse("file")
            .expect("single relative path should parse")
            .parent()
    );

    let uri = FsUri::parse("mock://user@example.com:8080/root/file.txt?region=test")
        .expect("URI should parse");
    let authority = uri.authority.expect("authority should exist");
    assert_eq!("mock", uri.scheme);
    assert_eq!("example.com", authority.host);
    assert_eq!(Some(8080), authority.port);
    assert_eq!(Some("user"), authority.username.as_deref());
    assert_eq!("/root/file.txt", uri.path.as_str());
    assert_eq!(Some(String::from("test")), uri.query.get("region"));
    assert!(FsUri::parse("not a uri").is_err());
    assert!(FsUri::parse("mock:").is_err());
    let no_authority = FsUri::parse("mock:/plain").expect("URI without authority should parse");
    assert!(no_authority.authority.is_none());
    assert_eq!("/plain", no_authority.path.as_str());
    let host_without_details =
        FsUri::parse("mock://bucket/root").expect("host-only URI should parse");
    let host_authority = host_without_details
        .authority
        .expect("authority should exist");
    assert_eq!("bucket", host_authority.host);
    assert_eq!(None, host_authority.port);
    assert_eq!(None, host_authority.username);

    let custom_authority = FsAuthority::new("bucket")
        .with_port(443)
        .with_username("alice");
    assert_eq!("bucket", custom_authority.host);
    assert_eq!(Some(443), custom_authority.port);
    assert_eq!(Some("alice"), custom_authority.username.as_deref());
    assert_eq!(None, FsAuthority::new("bucket").with_username("").username);
}

#[test]
fn test_error_metadata_and_options_are_usable() {
    let error = FsError::with_source(
        FsErrorKind::Io,
        FsOperation::Copy,
        "copy failed",
        std::io::Error::other("source"),
    )
    .with_path(FsPath::parse("/source").expect("path should parse"))
    .with_target(FsPath::parse("/target").expect("path should parse"))
    .with_provider("mock");
    assert_eq!(FsErrorKind::Io, error.kind());
    assert!(error.to_string().contains("copy failed"));
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(
        FsErrorKind::InvalidPath,
        FsError::invalid_path(FsOperation::ParsePath, "bad").kind()
    );

    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    assert_eq!("abc", checksum.value);
    let metadata = FileMetadata::new(FileType::Directory);
    assert!(metadata.is_directory_like());
    assert!(FileMetadata::new(FileType::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileType::File).is_directory_like());

    let entry = DirEntry::new(
        FsPath::parse("/dir/file.txt").expect("path should parse"),
        FileType::File,
    );
    assert_eq!("file.txt", entry.name);

    let fs_metadata = FileSystemMetadata::new("mock");
    assert_eq!("mock", fs_metadata.provider_id);
    assert_eq!(PathSemantics::Hierarchical, fs_metadata.path_semantics);

    let outcome = WriteOutcome::new();
    assert_eq!(None, outcome.bytes_written);

    assert_eq!(
        AtomicityRequirement::BestEffort,
        AtomicityRequirement::default()
    );
    assert_eq!(ChecksumPolicy::None, ChecksumPolicy::default());
    assert_eq!(CopyConflictPolicy::Fail, CopyConflictPolicy::default());
    assert_eq!(CopyMode::Auto, CopyMode::default());
    assert_eq!(
        MetadataPreservePolicy::Portable,
        MetadataPreservePolicy::default()
    );
    assert_eq!(ProgressPolicy::CountOnly, ProgressPolicy::default());
    assert_eq!(
        ServerSidePreference::Prefer,
        ServerSidePreference::default()
    );
    assert_eq!(WriteMode::CreateOrTruncate, WriteMode::default());

    let read_options = ReadOptions {
        offset: Some(1),
        length: Some(2),
        if_match: Some("a".to_owned()),
        if_none_match: Some("b".to_owned()),
        checksum: ChecksumPolicy::Required,
    };
    assert_eq!(Some(1), read_options.offset);

    let write_options = WriteOptions {
        create_parent: true,
        mode: WriteMode::ConditionalReplace {
            etag: "v1".to_owned(),
        },
        content_type: Some("text/plain".to_owned()),
        user_metadata: qubit_metadata::Metadata::new(),
        checksum: Some(checksum),
    };
    assert!(write_options.create_parent);

    let list_options = ListOptions {
        recursive: true,
        follow_symlinks: true,
        include_metadata: true,
        page_size: Some(10),
        prefix: Some("a".to_owned()),
    };
    assert!(list_options.recursive);

    let delete_options = DeleteOptions {
        recursive: true,
        missing_ok: true,
        if_match: Some("v1".to_owned()),
    };
    assert!(delete_options.missing_ok);

    let create_options = CreateDirOptions {
        recursive: true,
        exists_ok: true,
        user_metadata: qubit_metadata::Metadata::new(),
    };
    assert!(create_options.exists_ok);

    let rename_options = RenameOptions {
        overwrite: true,
        atomic: AtomicityRequirement::Required,
    };
    assert!(rename_options.overwrite);
    assert_eq!(
        RenameOptions {
            overwrite: false,
            atomic: AtomicityRequirement::BestEffort,
        },
        RenameOptions::default()
    );

    let persist_options = PersistOptions {
        overwrite: true,
        atomic: AtomicityRequirement::BestEffort,
        allow_copy_delete: true,
        preserve_metadata: MetadataPreservePolicy::All,
    };
    assert!(persist_options.allow_copy_delete);
}

#[test]
fn test_copy_types_cover_full_model() {
    let default_options = CopyOptions::default();
    assert_eq!(CopyMode::Auto, default_options.mode);
    assert_eq!(CopyMode::File, CopyOptions::file().mode);
    assert_eq!(CopyMode::Tree, CopyOptions::tree().mode);

    let options = CopyOptions {
        mode: CopyMode::Tree,
        conflict: CopyConflictPolicy::Skip,
        preserve_metadata: MetadataPreservePolicy::ProviderNative,
        server_side: ServerSidePreference::Disable,
        follow_symlinks: true,
        create_parent: true,
        continue_on_error: true,
        filter: None,
        progress: ProgressPolicy::Detailed,
    };
    assert!(options.follow_symlinks);

    let mut stats = CopyStats {
        files: 1,
        directories: 2,
        symlinks: 3,
        objects: 4,
        prefixes: 5,
        bytes: 6,
        overwritten: 7,
        skipped: 8,
        failed: 9,
    };
    stats.add_assign(&CopyStats {
        files: 10,
        directories: 20,
        symlinks: 30,
        objects: 40,
        prefixes: 50,
        bytes: 60,
        overwritten: 70,
        skipped: 80,
        failed: 90,
    });
    assert_eq!(11, stats.files);
    assert_eq!(22, stats.directories);
    assert_eq!(33, stats.symlinks);
    assert_eq!(44, stats.objects);
    assert_eq!(55, stats.prefixes);
    assert_eq!(66, stats.bytes);
    assert_eq!(77, stats.overwritten);
    assert_eq!(88, stats.skipped);
    assert_eq!(99, stats.failed);

    let outcome = CopyOutcome::new(stats, CopyMethod::Mixed);
    assert_eq!(CopyMethod::Mixed, outcome.method);
    assert_eq!(CopyMethod::Local, CopyMethod::Local);
    assert_eq!(CopyMethod::ServerSide, CopyMethod::ServerSide);
    assert_eq!(CopyMethod::Stream, CopyMethod::Stream);
}

#[test]
fn test_filesystem_traits_and_registry_work_together() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone());
    let path = FsPath::parse("/file.txt").expect("path should parse");

    assert!(fs.capabilities().directories);
    assert!(!fs.exists(&path).expect("exists should succeed"));
    fs.write_all(&path, b"data")
        .expect("write_all should succeed");
    assert!(fs.exists(&path).expect("exists should succeed"));
    assert_eq!(
        b"data".to_vec(),
        fs.read_all(&path).expect("read_all should succeed")
    );
    assert_eq!(
        4,
        fs.metadata(&path)
            .expect("metadata should exist")
            .len
            .unwrap()
    );

    let entries = fs
        .list(&FsPath::root(), &ListOptions::default())
        .expect("list should start")
        .collect_entries()
        .expect("collect should succeed");
    assert_eq!(1, entries.len());
    assert!(
        (Box::new(FailingDirectoryStream) as Box<dyn DirectoryStream>)
            .collect_entries()
            .is_err()
    );
    let empty_entries = (Box::new(MockDirectoryStream {
        entries: Vec::new(),
    }) as Box<dyn DirectoryStream>)
        .collect_entries()
        .expect("empty stream should collect");
    assert!(empty_entries.is_empty());
    assert!(
        (Box::new(PartiallyFailingDirectoryStream {
            entry: Some(DirEntry::new(
                FsPath::parse("/partial.txt").expect("path should parse"),
                FileType::File,
            )),
        }) as Box<dyn DirectoryStream>)
            .collect_entries()
            .is_err()
    );

    let mut cursor = Cursor::new(b"abc".to_vec());
    assert!(FileReader::metadata(&cursor).is_none());
    let mut buffer = Vec::new();
    cursor.read_to_end(&mut buffer).expect("cursor should read");
    assert_eq!(b"abc".to_vec(), buffer);

    let mut registry = FileSystemRegistry::new();
    registry
        .register(MockProvider { fs: fs.clone() })
        .expect("provider should register");
    assert_eq!(vec!["mock"], registry.provider_names());
    let opened = registry.open("mem:///file.txt").expect("alias should open");
    assert!(opened.capabilities().directories);
    assert!(registry.open("missing:///file.txt").is_err());

    let resolver = FileSystemResolver::new(Arc::new(registry));
    let resolved = resolver
        .resolve("mock:///file.txt")
        .expect("URI should resolve");
    assert_eq!("/file.txt", resolved.path.as_str());
    let ResolvedPath { filesystem, path } = resolved;
    assert!(filesystem.exists(&path).expect("resolved fs should work"));
    assert!(resolver.resolve("not a uri").is_err());

    let config = FileSystemConfig {
        uri: FsUri::parse("mock:///file.txt").expect("URI should parse"),
        options: qubit_metadata::Metadata::new(),
        credentials: Some(CredentialRef::DefaultChain),
    };
    assert!(matches!(
        config.credentials,
        Some(CredentialRef::DefaultChain)
    ));
    assert_eq!(
        CredentialRef::Profile("p".to_owned()),
        CredentialRef::Profile("p".to_owned()),
    );
    assert_eq!(
        CredentialRef::Environment {
            access_key: "AK".to_owned(),
            secret_key: "SK".to_owned(),
        },
        CredentialRef::Environment {
            access_key: "AK".to_owned(),
            secret_key: "SK".to_owned(),
        },
    );
    assert_eq!(
        CredentialRef::Provider("vault".to_owned()),
        CredentialRef::Provider("vault".to_owned()),
    );
}

#[test]
fn test_registry_maps_spi_errors() {
    let descriptor_errors = vec![
        ProviderRegistryError::EmptyProviderName,
        ProviderRegistryError::InvalidProviderName {
            name: "bad name".to_owned(),
            reason: "contains whitespace".to_owned(),
        },
        ProviderRegistryError::DuplicateProviderName {
            name: provider_name("duplicate"),
        },
        ProviderRegistryError::DuplicateProviderCandidate {
            name: provider_name("duplicate"),
        },
        ProviderRegistryError::UnknownProvider {
            name: provider_name("missing"),
        },
        ProviderRegistryError::ProviderUnavailable {
            name: provider_name("offline"),
            source: ProviderCreateError::unavailable("offline"),
        },
        ProviderRegistryError::ProviderCreate {
            name: provider_name("broken"),
            source: ProviderCreateError::failed("broken"),
        },
        ProviderRegistryError::NoAvailableProvider {
            failures: vec![ProviderFailure::unknown("missing").expect("failure should be valid")],
        },
        ProviderRegistryError::EmptyRegistry,
    ];

    for error in descriptor_errors {
        let mut registry = FileSystemRegistry::new();
        let mapped = registry
            .register(DescriptorErrorProvider { error })
            .expect_err("descriptor error should be mapped");
        assert!(matches!(
            mapped.kind(),
            FsErrorKind::InvalidPath | FsErrorKind::ProviderUnavailable | FsErrorKind::Other
        ));
    }

    let mut shared_registry = FileSystemRegistry::new();
    let shared: Arc<dyn ServiceProvider<FileSystemSpec>> = Arc::new(MockProvider {
        fs: MockFs::default(),
    });
    shared_registry
        .register_shared(shared)
        .expect("shared provider should register");
    assert_eq!(vec!["mock"], shared_registry.provider_names());

    let mut unavailable_registry = FileSystemRegistry::new();
    unavailable_registry
        .register(FailingCreateProvider {
            id: "offline",
            error: ProviderCreateError::unavailable("offline"),
        })
        .expect("provider should register");
    assert_eq!(
        FsErrorKind::ProviderUnavailable,
        unavailable_registry
            .open("offline:///file.txt")
            .expect_err("unavailable provider should fail")
            .kind()
    );

    let mut broken_registry = FileSystemRegistry::new();
    broken_registry
        .register(FailingCreateProvider {
            id: "broken",
            error: ProviderCreateError::failed("broken"),
        })
        .expect("provider should register");
    assert_eq!(
        FsErrorKind::Other,
        broken_registry
            .open("broken:///file.txt")
            .expect_err("broken provider should fail")
            .kind()
    );

    let empty_resolver = FileSystemResolver::new(Arc::new(FileSystemRegistry::new()));
    assert!(empty_resolver.resolve("missing:///file.txt").is_err());
    assert!(
        empty_resolver
            .resolve_uri(FsUri::parse("missing:///file.txt").expect("URI should parse"))
            .is_err()
    );
}

#[test]
fn test_filesystem_extension_error_branches() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone());
    let path = FsPath::parse("/file.txt").expect("path should parse");

    state.lock().expect("state lock should succeed").fail_read = true;
    assert!(fs.read_all(&path).is_err());
    state.lock().expect("state lock should succeed").fail_read = false;

    state.lock().expect("state lock should succeed").fail_write = true;
    assert!(fs.write_all(&path, b"data").is_err());
    assert_eq!(1, state.lock().expect("state lock should succeed").aborts);
}

#[test]
fn test_managed_temporary_resources_cover_cleanup_persist_keep_and_drop() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));

    let temp_file = TempResources::create_file(fs.clone(), &TempFileOptions::default())
        .expect("temp file should be created");
    let temp_path = temp_file.path().clone();
    assert!(fs.exists(&temp_path).expect("temp file should exist"));
    temp_file.cleanup().expect("cleanup should succeed");
    assert!(!fs.exists(&temp_path).expect("temp file should be gone"));

    let temp_dir = TempResources::create_dir(fs.clone(), &TempDirOptions::default())
        .expect("temp dir should be created");
    let temp_dir_path = temp_dir.path().clone();
    assert!(fs.exists(&temp_dir_path).expect("temp dir should exist"));
    temp_dir.cleanup().expect("cleanup should succeed");
    assert!(!fs.exists(&temp_dir_path).expect("temp dir should be gone"));

    let file_path = FsPath::parse("/manual-file.tmp").expect("path should parse");
    fs.write_all(&file_path, b"data")
        .expect("write should succeed");
    let kept_file = Box::new(ManagedTempFile::new(fs.clone(), file_path.clone()))
        .keep()
        .expect("keep should succeed");
    assert_eq!(file_path, kept_file);

    let dir_path = FsPath::parse("/manual-dir.tmp").expect("path should parse");
    fs.create_dir(&dir_path, &CreateDirOptions::default())
        .expect("dir should be created");
    let kept_dir = Box::new(ManagedTempDir::new(fs.clone(), dir_path.clone()))
        .keep()
        .expect("keep should succeed");
    assert_eq!(dir_path, kept_dir);

    let rename_file = FsPath::parse("/rename-file.tmp").expect("path should parse");
    fs.write_all(&rename_file, b"data")
        .expect("write should succeed");
    Box::new(ManagedTempFile::new(fs.clone(), rename_file))
        .persist(
            &FsPath::parse("/renamed-file.txt").expect("path should parse"),
            &PersistOptions::default(),
        )
        .expect("persist should rename");

    let rename_dir = FsPath::parse("/rename-dir.tmp").expect("path should parse");
    fs.create_dir(&rename_dir, &CreateDirOptions::default())
        .expect("dir should be created");
    Box::new(ManagedTempDir::new(fs.clone(), rename_dir))
        .persist(
            &FsPath::parse("/renamed-dir").expect("path should parse"),
            &PersistOptions::default(),
        )
        .expect("persist should rename");

    state
        .lock()
        .expect("state lock should succeed")
        .fail_rename_unsupported = true;
    let copy_file = FsPath::parse("/copy-file.tmp").expect("path should parse");
    fs.write_all(&copy_file, b"data")
        .expect("write should succeed");
    Box::new(ManagedTempFile::new(fs.clone(), copy_file))
        .persist(
            &FsPath::parse("/copy-file.txt").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                atomic: AtomicityRequirement::BestEffort,
                ..PersistOptions::default()
            },
        )
        .expect("persist should copy-delete");

    let copy_dir = FsPath::parse("/copy-dir.tmp").expect("path should parse");
    fs.create_dir(&copy_dir, &CreateDirOptions::default())
        .expect("dir should be created");
    Box::new(ManagedTempDir::new(fs.clone(), copy_dir))
        .persist(
            &FsPath::parse("/copy-dir").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                atomic: AtomicityRequirement::BestEffort,
                ..PersistOptions::default()
            },
        )
        .expect("persist should copy-delete");

    let drop_file = FsPath::parse("/drop-file.tmp").expect("path should parse");
    fs.write_all(&drop_file, b"data")
        .expect("write should succeed");
    drop(ManagedTempFile::new(fs.clone(), drop_file.clone()));
    assert!(!fs.exists(&drop_file).expect("drop cleanup should run"));

    let drop_dir = FsPath::parse("/drop-dir.tmp").expect("path should parse");
    fs.create_dir(&drop_dir, &CreateDirOptions::default())
        .expect("dir should be created");
    drop(ManagedTempDir::new(fs.clone(), drop_dir.clone()));
    assert!(!fs.exists(&drop_dir).expect("drop cleanup should run"));

    state.lock().expect("state lock should succeed").fail_delete = true;
    drop(ManagedTempFile::new(
        fs.clone(),
        FsPath::parse("/drop-error.tmp").expect("path should parse"),
    ));
    drop(ManagedTempDir::new(
        fs,
        FsPath::parse("/drop-dir-error.tmp").expect("path should parse"),
    ));
}

#[test]
fn test_temporary_resources_cover_custom_paths_and_failure_branches() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    let parent = FsPath::parse("/tmp").expect("path should parse");
    fs.create_dir(&parent, &CreateDirOptions::default())
        .expect("parent should be created");

    let temp_file = TempResources::create_file(
        fs.clone(),
        &TempFileOptions {
            parent: Some(parent.clone()),
            prefix: "pre-".to_owned(),
            suffix: ".tmp".to_owned(),
        },
    )
    .expect("custom temp file should be created");
    assert!(temp_file.path().as_str().starts_with("/tmp/pre-"));
    assert!(temp_file.path().as_str().ends_with(".tmp"));
    temp_file.cleanup().expect("custom temp file should clean");

    let temp_dir = TempResources::create_dir(
        fs.clone(),
        &TempDirOptions {
            parent: Some(parent),
            prefix: "dir-".to_owned(),
            suffix: ".work".to_owned(),
        },
    )
    .expect("custom temp dir should be created");
    assert!(temp_dir.path().as_str().contains("/tmp/dir-"));
    assert!(temp_dir.path().as_str().ends_with(".work"));
    temp_dir.cleanup().expect("custom temp dir should clean");

    assert!(
        TempResources::create_file(
            fs.clone(),
            &TempFileOptions {
                parent: None,
                prefix: "../".to_owned(),
                suffix: String::new(),
            },
        )
        .is_err()
    );

    let commit_error_state = Arc::new(Mutex::new(MockState {
        fail_commit: true,
        ..MockState::default()
    }));
    let commit_error_fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(commit_error_state));
    assert!(TempResources::create_file(commit_error_fs, &TempFileOptions::default()).is_err());

    let create_dir_error_state = Arc::new(Mutex::new(MockState {
        fail_create_dir: true,
        ..MockState::default()
    }));
    let create_dir_error_fs: Arc<dyn FileSystem> =
        Arc::new(MockFs::with_state(create_dir_error_state));
    assert!(TempResources::create_dir(create_dir_error_fs, &TempDirOptions::default()).is_err());

    let cleanup_file = FsPath::parse("/cleanup-error.tmp").expect("path should parse");
    fs.write_all(&cleanup_file, b"data")
        .expect("file should be created");
    state.lock().expect("state lock should succeed").fail_delete = true;
    assert!(
        Box::new(ManagedTempFile::new(fs.clone(), cleanup_file))
            .cleanup()
            .is_err()
    );

    let cleanup_dir = FsPath::parse("/cleanup-dir-error.tmp").expect("path should parse");
    fs.create_dir(&cleanup_dir, &CreateDirOptions::default())
        .expect("dir should be created");
    assert!(
        Box::new(ManagedTempDir::new(fs.clone(), cleanup_dir))
            .cleanup()
            .is_err()
    );
    state.lock().expect("state lock should succeed").fail_delete = false;

    state
        .lock()
        .expect("state lock should succeed")
        .fail_rename_unsupported = true;
    let unsupported_file = FsPath::parse("/unsupported-file.tmp").expect("path should parse");
    fs.write_all(&unsupported_file, b"data")
        .expect("file should be created");
    assert!(
        Box::new(ManagedTempFile::new(fs.clone(), unsupported_file))
            .persist(
                &FsPath::parse("/unsupported-file.txt").expect("path should parse"),
                &PersistOptions::default(),
            )
            .is_err()
    );

    let unsupported_dir = FsPath::parse("/unsupported-dir.tmp").expect("path should parse");
    fs.create_dir(&unsupported_dir, &CreateDirOptions::default())
        .expect("dir should be created");
    assert!(
        Box::new(ManagedTempDir::new(fs.clone(), unsupported_dir))
            .persist(
                &FsPath::parse("/unsupported-dir").expect("path should parse"),
                &PersistOptions::default(),
            )
            .is_err()
    );

    let required_copy_file = FsPath::parse("/required-copy.tmp").expect("path should parse");
    fs.write_all(&required_copy_file, b"data")
        .expect("file should be created");
    Box::new(ManagedTempFile::new(fs.clone(), required_copy_file))
        .persist(
            &FsPath::parse("/required-copy.txt").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                ..PersistOptions::default()
            },
        )
        .expect("required fallback copy should succeed");
}
