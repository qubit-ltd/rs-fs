// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
// qubit-style: allow test-file-name -- this module is included by
// handle_support/mod.rs.
use std::io::Cursor;
use std::io::Result as IoResult;
use std::sync::Arc;
use std::sync::Mutex;

use qubit_fs::AchievedAtomicity;
use qubit_fs::CopyOptions;
use qubit_fs::CreateDirectoryOptions;
use qubit_fs::CreateDirectoryOutcome;
use qubit_fs::DeleteOptions;
use qubit_fs::DeleteOutcome;
use qubit_fs::DirEntry;
use qubit_fs::FileKind;
use qubit_fs::FileMetadata;
use qubit_fs::FileSystem;
use qubit_fs::FileSystemCapabilities;
use qubit_fs::FileSystemCapability;
use qubit_fs::FileSystemId;
use qubit_fs::FileSystemInfo;
use qubit_fs::FileSystemLimit;
use qubit_fs::FileSystemLimits;
use qubit_fs::FileSystemProperties;
use qubit_fs::FsError;
use qubit_fs::FsErrorKind;
use qubit_fs::FsOperation;
use qubit_fs::FsResult;
use qubit_fs::ListOptions;
use qubit_fs::OpenedFileInfo;
use qubit_fs::Path;
use qubit_fs::PathConstraints;
use qubit_fs::PathSemantics;
use qubit_fs::PersistFailureState;
use qubit_fs::PersistOutcome;
use qubit_fs::PublicationMethod;
use qubit_fs::ReadOptions;
use qubit_fs::RenameFailureState;
use qubit_fs::RenameOutcome;
use qubit_fs::SymlinkPolicy;
use qubit_fs::TempDirectoryOptions;
use qubit_fs::TempFileOptions;
use qubit_fs::WriteAbortOutcome;
use qubit_fs::WriteFailureState;
use qubit_fs::WriteOptions;
use qubit_fs::WriteOutcome;
use qubit_fs::spi::CreateDirectoryRequest;
use qubit_fs::spi::CreateTempDirectoryRequest;
use qubit_fs::spi::CreateTempFileRequest;
use qubit_fs::spi::DeleteDirectoryRequest;
use qubit_fs::spi::DeleteFileRequest;
use qubit_fs::spi::DirectoryStreamSpi;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::FileWriterSpi;
use qubit_fs::spi::ListRequest;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenWriterRequest;
use qubit_fs::spi::OpenedDirectoryStream;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::OpenedTempDirectory;
use qubit_fs::spi::OpenedTempFile;
use qubit_fs::spi::OpenedWriter;
use qubit_fs::spi::PersistRequest;
use qubit_fs::spi::RenameRequest;
use qubit_fs::spi::SpiPersistFailure;
use qubit_fs::spi::SpiRenameFailure;
use qubit_fs::spi::SpiWriteFailure;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_fs::spi::TempResourceSpi;
use qubit_io::Input;
use qubit_io::Output;

pub(crate) struct BehaviorSpi {
    pub(crate) fail_commit: bool,
    pub(crate) fail_write: bool,
    pub(crate) commit_failure: Option<WriteFailureState>,
    pub(crate) abort_failure: Option<FsErrorKind>,
    pub(crate) limits: FileSystemLimits,
    pub(crate) entries: Mutex<Vec<DirEntry>>,
    pub(crate) cleanup_calls: Arc<Mutex<usize>>,
    pub(crate) persist_calls: Arc<Mutex<usize>>,
    pub(crate) temp_path: Path,
    pub(crate) temp_failure: Option<PersistFailureState>,
    pub(crate) temp_keep_error: Option<FsErrorKind>,
    pub(crate) temp_cleanup_error: Option<FsErrorKind>,
    pub(crate) directory_persist_non_atomic: bool,
    pub(crate) provider_open_error: bool,
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
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(entries),
        cleanup_calls: Arc::clone(&cleanup_calls),
        persist_calls: Arc::clone(&persist_calls),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
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
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown()
            .with_max_write_bytes(FileSystemLimit::Maximum(maximum)),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}
pub(crate) fn stream_failure_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: true,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}

/// Builds a filesystem whose handle-opening and temporary-creation calls fail
/// at the provider boundary after facade validation has succeeded.
pub(crate) fn provider_open_failure_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: true,
    })
    .expect("facade should construct")
}

/// Builds a filesystem with explicit provider write lifecycle failures.
pub(crate) fn writer_lifecycle_filesystem(
    commit_failure: Option<WriteFailureState>,
    abort_failure: Option<FsErrorKind>,
) -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure,
        abort_failure,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}
pub(crate) fn invalid_temp_path_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("relative").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}

/// Builds a filesystem whose temporary-file metadata claims a directory kind.
pub(crate) fn wrong_temp_kind_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/wrong-kind").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}

/// Builds a filesystem whose temporary identities violate the provider binding
/// and whose attempted cleanup also fails.
fn invalid_temp_cleanup_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/foreign").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: Some(FsErrorKind::Io),
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct")
}
/// Builds a filesystem whose temporary-resource persist operation reports
/// the supplied provider-confirmed partial-progress state.
pub(crate) fn temp_failure_filesystem(
    state: PersistFailureState,
) -> (FileSystem, Arc<Mutex<usize>>, Arc<Mutex<usize>>) {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let persist_calls = Arc::new(Mutex::new(0));
    let filesystem = FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::clone(&cleanup_calls),
        persist_calls: Arc::clone(&persist_calls),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: Some(state),
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct");
    (filesystem, cleanup_calls, persist_calls)
}
/// Builds a filesystem whose temporary-resource lifecycle callbacks fail with
/// the configured error kinds.
pub(crate) fn temp_lifecycle_error_filesystem(
    keep_error: Option<FsErrorKind>,
    cleanup_error: Option<FsErrorKind>,
) -> (FileSystem, Arc<Mutex<usize>>) {
    let cleanup_calls = Arc::new(Mutex::new(0));
    let filesystem = FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::clone(&cleanup_calls),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: keep_error,
        temp_cleanup_error: cleanup_error,
        directory_persist_non_atomic: false,
        provider_open_error: false,
    })
    .expect("facade should construct");
    (filesystem, cleanup_calls)
}
/// Builds a filesystem whose temporary directory reports a non-atomic
/// persistence outcome while still advertising atomic capability support.
pub(crate) fn non_atomic_temp_directory_filesystem() -> FileSystem {
    FileSystem::from_spi(BehaviorSpi {
        fail_commit: false,
        fail_write: false,
        commit_failure: None,
        abort_failure: None,
        limits: FileSystemLimits::unknown(),
        entries: Mutex::new(Vec::new()),
        cleanup_calls: Arc::new(Mutex::new(0)),
        persist_calls: Arc::new(Mutex::new(0)),
        temp_path: Path::parse("/temporary").expect("test path should parse"),
        temp_failure: None,
        temp_keep_error: None,
        temp_cleanup_error: None,
        directory_persist_non_atomic: true,
        provider_open_error: false,
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

/// Exercises synchronous facade dispatch for directory creation and both
/// deletion primitives against the recording provider.
#[test]
fn test_handle_support_dispatches_directory_and_delete_operations() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    let path = Path::parse("/target").expect("test path should parse");

    assert!(
        !file_system
            .create_directory(&path, CreateDirectoryOptions::default())
            .expect("directory creation should succeed")
            .already_existed()
    );
    assert!(
        !file_system
            .delete_file(&path, DeleteOptions::default())
            .expect("file deletion should succeed")
            .already_missing()
    );
    assert!(
        !file_system
            .delete_directory(&path, DeleteOptions::default())
            .expect("directory deletion should succeed")
            .already_missing()
    );
}

/// Exercises each synchronous facade entry point on the successful provider
/// path so that its public validation and handle-binding contracts remain
/// covered together.
#[test]
fn test_handle_support_dispatches_successful_facade_operations() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    let source = Path::parse("/source").expect("test path should parse");
    let target = Path::parse("/target").expect("test path should parse");

    assert!(file_system.exists(&source).expect("stat should succeed"));
    let mut directory = file_system
        .list(&source, ListOptions::default())
        .expect("list should succeed");
    assert!(
        directory
            .next_entry()
            .expect("stream should succeed")
            .is_none()
    );

    let mut reader = file_system
        .open_reader(&source, ReadOptions::default())
        .expect("reader should open");
    let mut bytes = [0_u8; 5];
    assert_eq!(
        5,
        Input::read(&mut reader, &mut bytes)
            .expect("reader should transfer bytes")
    );

    let mut writer = file_system
        .open_writer(&target, WriteOptions::default())
        .expect("writer should open");
    Output::write_fully(&mut writer, b"bytes")
        .expect("writer should accept bytes");
    writer.commit().expect("writer should commit");

    let mut temporary_file = file_system
        .create_temp_file(TempFileOptions::default())
        .expect("temporary file should open");
    temporary_file
        .keep()
        .expect("temporary file should be kept");
    let mut temporary_directory = file_system
        .create_temp_directory(TempDirectoryOptions::default())
        .expect("temporary directory should open");
    temporary_directory
        .keep()
        .expect("temporary directory should be kept");
}

/// Invokes the synchronous SPI's optional default copy method and completes
/// the facade-owned stream fallback.
#[test]
fn test_handle_support_uses_default_spi_copy_decline() {
    let (file_system, _, _) = filesystem(false, Vec::new());
    let outcome = file_system
        .copy(
            &Path::parse("/source").expect("test path should parse"),
            &Path::parse("/target").expect("test path should parse"),
            CopyOptions::default(),
        )
        .expect("fallback should use reader and writer capabilities");
    assert!(outcome.used_fallback());
}

/// Ensures provider open failures are enriched by each synchronous facade
/// entry point after its local validation has completed.
#[test]
fn test_handle_support_enriches_open_and_temp_provider_failures() {
    let file_system = provider_open_failure_filesystem();
    let path = Path::parse("/target").expect("test path should parse");
    for error in [
        file_system
            .list(&path, ListOptions::default())
            .expect_err("list provider failure should propagate"),
        file_system
            .open_writer(&path, WriteOptions::default())
            .expect_err("writer provider failure should propagate"),
        file_system
            .open_reader(&path, ReadOptions::default())
            .expect_err("reader provider failure should propagate"),
        file_system
            .create_temp_file(TempFileOptions::default())
            .expect_err("temporary-file provider failure should propagate"),
        file_system
            .create_temp_directory(TempDirectoryOptions::default())
            .expect_err(
                "temporary-directory provider failure should propagate",
            ),
    ] {
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some("handles-test"), error.provider());
    }
}

/// Verifies pathless temporary provider failures do not receive a fabricated
/// hierarchical root path.
#[test]
fn test_handle_support_keeps_pathless_temp_errors_pathless() {
    let file_system = provider_open_failure_filesystem();
    let file_error = file_system
        .create_temp_file(TempFileOptions::default())
        .expect_err("temporary-file provider failure should propagate");
    let directory_error = file_system
        .create_temp_directory(TempDirectoryOptions::default())
        .expect_err("temporary-directory provider failure should propagate");
    for error in [file_error, directory_error] {
        assert_eq!(FsOperation::CreateTemp, error.operation());
        assert_eq!(None, error.path());
        assert_eq!(Some("handles-test"), error.provider());
    }
}

/// Verifies temporary creation rejects an invalid parent before provider I/O.
#[test]
fn test_handle_support_validates_temp_parent_before_provider_call() {
    let file_system = provider_open_failure_filesystem();
    let parent =
        Path::parse("relative").expect("relative test path should parse");
    let file_error = file_system
        .create_temp_file(
            TempFileOptions::default().with_parent(Some(parent.clone())),
        )
        .expect_err("invalid temporary-file parent must fail in the facade");
    assert_eq!(FsErrorKind::InvalidPath, file_error.kind());
    assert_eq!(FsOperation::CreateTemp, file_error.operation());
    assert_eq!(Some(&parent), file_error.path());

    let directory_error = file_system
        .create_temp_directory(
            TempDirectoryOptions::default().with_parent(Some(parent.clone())),
        )
        .expect_err(
            "invalid temporary-directory parent must fail in the facade",
        );
    assert_eq!(FsErrorKind::InvalidPath, directory_error.kind());
    assert_eq!(FsOperation::CreateTemp, directory_error.operation());
    assert_eq!(Some(&parent), directory_error.path());
}

/// Rejects invalid temporary identities even when the provider also fails to
/// clean up the invalid resource.
#[test]
fn test_handle_support_rejects_invalid_temp_identities_with_cleanup_failure() {
    let file_system = invalid_temp_cleanup_filesystem();
    for error in [
        file_system
            .create_temp_file(TempFileOptions::default())
            .expect_err("foreign temporary file identity must be rejected"),
        file_system
            .create_temp_directory(TempDirectoryOptions::default())
            .expect_err(
                "foreign temporary directory identity must be rejected",
            ),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
        assert_eq!(FsOperation::CreateTemp, error.operation());
        assert!(
            error.source().is_some_and(|source| source
                .to_string()
                .contains("injected temporary cleanup failure")),
            "cleanup failure must remain the inspectable source"
        );
    }
}

/// Rejects invalid temporary paths after a successful provider cleanup for both
/// temporary resource kinds.
#[test]
fn test_handle_support_rejects_invalid_temp_paths_after_cleanup() {
    for error in [
        invalid_temp_path_filesystem()
            .create_temp_file(TempFileOptions::default())
            .expect_err("relative temporary file path must be rejected"),
        invalid_temp_path_filesystem()
            .create_temp_directory(TempDirectoryOptions::default())
            .expect_err("relative temporary directory path must be rejected"),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    }
}

/// Rejects a temporary-file envelope whose metadata claims a directory.
#[test]
fn test_handle_support_rejects_wrong_temp_kind() {
    let error = wrong_temp_kind_filesystem()
        .create_temp_file(TempFileOptions::default())
        .expect_err("temporary-file kind must be validated");
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
}

impl BehaviorSpi {
    fn unsupported() -> FsError {
        FsError::new(
            FsErrorKind::UnsupportedOperation,
            FsOperation::Other,
            "unused test operation",
        )
    }
    fn info(&self, kind: FileKind) -> OpenedFileInfo {
        OpenedFileInfo::new(
            FileSystemId::new(if self.temp_path.as_str() == "/foreign" {
                "foreign"
            } else {
                "handles-test"
            })
            .expect("valid test id"),
            self.temp_path.clone(),
        )
        .with_metadata(FileMetadata::new(kind))
    }
}
impl FileSystemSpi for BehaviorSpi {
    fn properties(&self) -> FileSystemProperties {
        FileSystemProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("handles-test").expect("valid test id"),
                "handles-test",
                PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with_guaranteed(FileSystemCapability::List)
                .with_guaranteed(FileSystemCapability::Copy)
                .with_guaranteed(FileSystemCapability::Read)
                .with_guaranteed(FileSystemCapability::Write)
                .with_guaranteed(FileSystemCapability::CreateDirectory)
                .with_guaranteed(FileSystemCapability::Delete)
                .with_guaranteed(FileSystemCapability::AtomicReplace)
                .with_guaranteed(FileSystemCapability::TempFile)
                .with_guaranteed(FileSystemCapability::TempDirectory)
                .with_guaranteed(FileSystemCapability::AtomicTempPersist),
            self.limits,
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid test properties")
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        let _ = request.options();
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
    fn list(
        &self,
        request: ListRequest<'_>,
    ) -> FsResult<OpenedDirectoryStream> {
        let _ = request.path();
        let _ = request.options();
        if self.provider_open_error {
            return Err(Self::unsupported());
        }
        Ok(OpenedDirectoryStream::new(Box::new(Entries(
            std::mem::take(
                &mut *self.entries.lock().expect("entries lock should succeed"),
            ),
        ))))
    }
    fn open_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedReader> {
        if self.provider_open_error {
            return Err(Self::unsupported());
        }
        let _ = request.options();
        Ok(OpenedReader::new(
            OpenedFileInfo::new(
                FileSystemId::new("handles-test").expect("valid test id"),
                request.path().clone(),
            ),
            Box::new(Cursor::new(b"bytes".to_vec())),
        ))
    }
    fn open_writer(
        &self,
        request: OpenWriterRequest<'_>,
    ) -> FsResult<OpenedWriter> {
        let _ = request.options();
        if self.provider_open_error {
            return Err(Self::unsupported());
        }
        Ok(OpenedWriter::new(
            OpenedFileInfo::new(
                FileSystemId::new("handles-test").expect("valid test id"),
                request.path().clone(),
            ),
            Box::new(Writer {
                fail_commit: self.fail_commit,
                fail_write: self.fail_write,
                commit_failure: self.commit_failure,
                abort_failure: self.abort_failure,
                non_atomic_commit: self.directory_persist_non_atomic,
            }),
        ))
    }
    fn create_directory(
        &self,
        request: CreateDirectoryRequest<'_>,
    ) -> FsResult<CreateDirectoryOutcome> {
        let _ = request.path();
        let _ = request.options();
        Ok(CreateDirectoryOutcome::new(false))
    }
    fn delete_file(
        &self,
        request: DeleteFileRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        let _ = request.path();
        let _ = request.options();
        Ok(DeleteOutcome::new(false))
    }
    fn delete_directory(
        &self,
        request: DeleteDirectoryRequest<'_>,
    ) -> FsResult<DeleteOutcome> {
        let _ = request.path();
        let _ = request.options();
        Ok(DeleteOutcome::new(false))
    }
    fn rename(
        &self,
        request: RenameRequest<'_>,
    ) -> Result<RenameOutcome, SpiRenameFailure> {
        let _ = request.source();
        let _ = request.target();
        let _ = request.options();
        Err(SpiRenameFailure::new(
            Self::unsupported(),
            RenameFailureState::Unchanged,
        ))
    }
    fn create_temp_file(
        &self,
        request: CreateTempFileRequest,
    ) -> FsResult<OpenedTempFile> {
        let _ = request.options();
        if self.provider_open_error {
            return Err(Self::unsupported());
        }
        Ok(OpenedTempFile::new(
            self.info(if self.temp_path.as_str() == "/wrong-kind" {
                FileKind::Directory
            } else {
                FileKind::File
            }),
            Box::new(Temp {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                persist_calls: Arc::clone(&self.persist_calls),
                non_atomic: true,
                failure: self.temp_failure,
                keep_error: self.temp_keep_error,
                cleanup_error: self.temp_cleanup_error,
            }),
        ))
    }
    fn create_temp_directory(
        &self,
        request: CreateTempDirectoryRequest,
    ) -> FsResult<OpenedTempDirectory> {
        let _ = request.options();
        if self.provider_open_error {
            return Err(Self::unsupported());
        }
        Ok(OpenedTempDirectory::new(
            self.info(FileKind::Directory),
            Box::new(Temp {
                cleanup_calls: Arc::clone(&self.cleanup_calls),
                persist_calls: Arc::clone(&self.persist_calls),
                non_atomic: self.directory_persist_non_atomic,
                failure: self.temp_failure,
                keep_error: self.temp_keep_error,
                cleanup_error: self.temp_cleanup_error,
            }),
        ))
    }
}
struct Writer {
    fail_commit: bool,
    fail_write: bool,
    commit_failure: Option<WriteFailureState>,
    abort_failure: Option<FsErrorKind>,
    non_atomic_commit: bool,
}
impl Output for Writer {
    type Item = u8;
    unsafe fn write_unchecked(
        &mut self,
        _: &[u8],
        _: usize,
        count: usize,
    ) -> IoResult<usize> {
        if self.fail_write {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "stream secret=top-secret",
            ))
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
        if let Some(state) = self.commit_failure {
            return Err(SpiWriteFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::CommitWriter,
                    "injected commit failure",
                ),
                state,
            ));
        }
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
                if self.non_atomic_commit {
                    AchievedAtomicity::NonAtomic
                } else {
                    AchievedAtomicity::Atomic
                },
                PublicationMethod::Direct,
            ))
        }
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        match self.abort_failure {
            Some(kind) => Err(FsError::new(
                kind,
                FsOperation::AbortWriter,
                "injected abort failure",
            )),
            None => Ok(match self.commit_failure {
                Some(WriteFailureState::Published) => {
                    WriteAbortOutcome::Published
                }
                Some(WriteFailureState::Indeterminate) => {
                    WriteAbortOutcome::Indeterminate
                }
                Some(WriteFailureState::RetryableNotPublished)
                | Some(WriteFailureState::NotPublished)
                | None => WriteAbortOutcome::NotPublished,
            }),
        }
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
    failure: Option<PersistFailureState>,
    keep_error: Option<FsErrorKind>,
    cleanup_error: Option<FsErrorKind>,
}
impl TempResourceSpi for Temp {
    fn persist(
        &mut self,
        request: PersistRequest<'_>,
    ) -> Result<PersistOutcome, SpiPersistFailure> {
        *self
            .persist_calls
            .lock()
            .expect("persist lock should succeed") += 1;
        if let Some(state) = self.failure {
            return Err(SpiPersistFailure::new(
                FsError::new(
                    FsErrorKind::Io,
                    FsOperation::PersistTemp,
                    "injected temporary persist failure",
                ),
                state,
            ));
        }
        let target = if request.target().as_str() == "/wrong-persist-target" {
            Path::parse("/reported-persist-target")
                .expect("generated path should parse")
        } else {
            request.target().clone()
        };
        Ok(PersistOutcome::new(
            target,
            if self.non_atomic {
                AchievedAtomicity::NonAtomic
            } else {
                AchievedAtomicity::Atomic
            },
            PublicationMethod::Direct,
        ))
    }
    fn keep(&mut self) -> FsResult<()> {
        self.keep_error.map_or(Ok(()), |kind| {
            Err(FsError::new(
                kind,
                FsOperation::KeepTemp,
                "injected temporary keep failure",
            ))
        })
    }
    fn cleanup(&mut self) -> FsResult<()> {
        *self
            .cleanup_calls
            .lock()
            .expect("cleanup lock should succeed") += 1;
        self.cleanup_error.map_or(Ok(()), |kind| {
            Err(FsError::new(
                kind,
                FsOperation::CleanupTemp,
                "injected temporary cleanup failure",
            ))
        })
    }
}
