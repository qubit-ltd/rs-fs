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
use std::io::{
    Cursor,
    Result as IoResult,
};
use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::spi::{
    CreateDirectoryRequest,
    CreateTempDirectoryRequest,
    CreateTempFileRequest,
    DeleteDirectoryRequest,
    DeleteFileRequest,
    DirectoryStreamSpi,
    FileSystemSpi,
    FileWriterSpi,
    ListRequest,
    OpenReaderRequest,
    OpenWriterRequest,
    OpenedDirectoryStream,
    OpenedReader,
    OpenedTempDirectory,
    OpenedTempFile,
    OpenedWriter,
    PersistRequest,
    RenameRequest,
    SpiPersistFailure,
    SpiRenameFailure,
    SpiWriteFailure,
    StatRequest,
    StatResponse,
    TempResourceSpi,
};
use qubit_fs::{
    AchievedAtomicity,
    CreateDirectoryOutcome,
    DeleteOutcome,
    DirEntry,
    FileKind,
    FileMetadata,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimits,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    OpenedFileInfo,
    Path,
    PathConstraints,
    PersistFailureState,
    PersistOutcome,
    PublicationMethod,
    RenameFailureState,
    RenameOutcome,
    SymlinkPolicy,
    WriteFailureState,
    WriteOutcome,
};
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
            .with_max_write_bytes(qubit_fs::FileSystemLimit::Maximum(maximum)),
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
            .create_directory(
                &path,
                qubit_fs::CreateDirectoryOptions::default()
            )
            .expect("directory creation should succeed")
            .already_existed()
    );
    assert!(
        !file_system
            .delete_file(&path, qubit_fs::DeleteOptions::default())
            .expect("file deletion should succeed")
            .already_missing()
    );
    assert!(
        !file_system
            .delete_directory(&path, qubit_fs::DeleteOptions::default())
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
        .list(&source, qubit_fs::ListOptions::default())
        .expect("list should succeed");
    assert!(
        directory
            .next_entry()
            .expect("stream should succeed")
            .is_none()
    );

    let mut reader = file_system
        .open_reader(&source, qubit_fs::ReadOptions::default())
        .expect("reader should open");
    let mut bytes = [0_u8; 5];
    assert_eq!(
        5,
        qubit_io::Input::read(&mut reader, &mut bytes)
            .expect("reader should transfer bytes")
    );

    let mut writer = file_system
        .open_writer(&target, qubit_fs::WriteOptions::default())
        .expect("writer should open");
    qubit_io::Output::write_fully(&mut writer, b"bytes")
        .expect("writer should accept bytes");
    writer.commit().expect("writer should commit");

    let mut temporary_file = file_system
        .create_temp_file(qubit_fs::TempFileOptions::default())
        .expect("temporary file should open");
    temporary_file
        .keep()
        .expect("temporary file should be kept");
    let mut temporary_directory = file_system
        .create_temp_directory(qubit_fs::TempDirectoryOptions::default())
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
            qubit_fs::CopyOptions::default(),
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
            .list(&path, qubit_fs::ListOptions::default())
            .expect_err("list provider failure should propagate"),
        file_system
            .open_writer(&path, qubit_fs::WriteOptions::default())
            .expect_err("writer provider failure should propagate"),
        file_system
            .open_reader(&path, qubit_fs::ReadOptions::default())
            .expect_err("reader provider failure should propagate"),
        file_system
            .create_temp_file(qubit_fs::TempFileOptions::default())
            .expect_err("temporary-file provider failure should propagate"),
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default())
            .expect_err(
                "temporary-directory provider failure should propagate",
            ),
    ] {
        assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
        assert_eq!(Some("handles-test"), error.provider());
    }
}

/// Rejects invalid temporary identities even when the provider also fails to
/// clean up the invalid resource.
#[test]
fn test_handle_support_rejects_invalid_temp_identities_with_cleanup_failure() {
    let file_system = invalid_temp_cleanup_filesystem();
    for error in [
        file_system
            .create_temp_file(qubit_fs::TempFileOptions::default())
            .expect_err("foreign temporary file identity must be rejected"),
        file_system
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default())
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
            .create_temp_file(qubit_fs::TempFileOptions::default())
            .expect_err("relative temporary file path must be rejected"),
        invalid_temp_path_filesystem()
            .create_temp_directory(qubit_fs::TempDirectoryOptions::default())
            .expect_err("relative temporary directory path must be rejected"),
    ] {
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    }
}

/// Rejects a temporary-file envelope whose metadata claims a directory.
#[test]
fn test_handle_support_rejects_wrong_temp_kind() {
    let error = wrong_temp_kind_filesystem()
        .create_temp_file(qubit_fs::TempFileOptions::default())
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
                qubit_fs::PathSemantics::Hierarchical,
            ),
            FileSystemCapabilities::new()
                .with(FileSystemCapability::List)
                .with(FileSystemCapability::Copy)
                .with(FileSystemCapability::Read)
                .with(FileSystemCapability::Write)
                .with(FileSystemCapability::CreateDirectory)
                .with(FileSystemCapability::Delete)
                .with(FileSystemCapability::AtomicReplace)
                .with(FileSystemCapability::TempFile)
                .with(FileSystemCapability::TempDirectory)
                .with(FileSystemCapability::AtomicTempPersist),
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
            FileMetadata::new(qubit_fs::FileKind::File),
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
    fn abort(&mut self) -> FsResult<qubit_fs::WriteAbortOutcome> {
        match self.abort_failure {
            Some(kind) => Err(FsError::new(
                kind,
                FsOperation::AbortWriter,
                "injected abort failure",
            )),
            None => Ok(match self.commit_failure {
                Some(WriteFailureState::Published) => {
                    qubit_fs::WriteAbortOutcome::Published
                }
                Some(WriteFailureState::Indeterminate) => {
                    qubit_fs::WriteAbortOutcome::Indeterminate
                }
                Some(WriteFailureState::RetryableNotPublished)
                | Some(WriteFailureState::NotPublished)
                | None => qubit_fs::WriteAbortOutcome::NotPublished,
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
    ) -> Result<PersistOutcome, qubit_fs::spi::SpiPersistFailure> {
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
