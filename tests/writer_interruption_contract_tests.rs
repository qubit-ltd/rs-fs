// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Regression coverage for write interruption retry boundaries.

use std::io;
use std::io::Cursor;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use qubit_fs::FileSystem;
use qubit_fs::FsError;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::copy::{CopyFailureState, CopyOptions};
use qubit_fs::error::{FsEffectState, FsErrorKind, FsOperation};
use qubit_fs::metadata::{
    AchievedAtomicity, FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability,
    FileSystemId, FileSystemInfo, FileSystemLimits, OpenedFileInfo, PublicationMethod,
    SymlinkPolicy, WriteOutcome,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::spi::{
    FileSystemSpi, FileWriterSpi, OpenReaderRequest, OpenWriterRequest, OpenedReader, OpenedWriter,
    ProviderOperation, ProviderOperations, ProviderProperties, SpiWriteFailure, StatRequest,
    StatResponse,
};
use qubit_fs::write::{WriteAbortOutcome, WriteOptions, WriterRecovery, WriterState};
use qubit_io::Output;

const ID: &str = "writer-interruption";

struct Provider(
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    FsErrorKind,
    FsEffectState,
    bool,
    bool,
);
struct Writer(
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
    FsErrorKind,
    FsEffectState,
    bool,
    bool,
);

fn properties() -> ProviderProperties {
    ProviderProperties::new(
        FileSystemInfo::new(
            FileSystemId::new(ID).unwrap(),
            "writer interruption",
            PathSemantics::Hierarchical,
        ),
        ProviderOperations::new()
            .with(ProviderOperation::Stat)
            .with(ProviderOperation::OpenWriter)
            .with(ProviderOperation::OpenReader),
        FileSystemCapabilities::new()
            .with_guaranteed(FileSystemCapability::Write)
            .with_guaranteed(FileSystemCapability::Read),
        FileSystemLimits::unknown(),
        PathConstraints::absolute(),
        SymlinkPolicy::Reject,
    )
    .unwrap()
}

impl FileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        properties()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        if request.path().as_str() == "/target" {
            return Err(
                FsError::new(FsErrorKind::NotFound, FsOperation::Stat, "missing target")
                    .with_effect_state(FsEffectState::Unchanged),
            );
        }
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File).with_len(Some(7)),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        let info = OpenedFileInfo::new(FileSystemId::new(ID)?, request.path().clone())
            .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(7)));
        Ok(OpenedReader::new(
            info,
            Box::new(Cursor::new(b"payload".to_vec())),
        ))
    }
    fn open_writer(&self, request: OpenWriterRequest<'_>) -> FsResult<OpenedWriter> {
        let info = OpenedFileInfo::new(FileSystemId::new(ID)?, request.path().clone());
        Ok(OpenedWriter::new(
            info,
            Box::new(Writer(
                Arc::clone(&self.0),
                Arc::clone(&self.1),
                self.2,
                self.3,
                self.4,
                self.5,
            )),
        ))
    }
}

impl Output for Writer {
    type Item = u8;
    unsafe fn write_unchecked(
        &mut self,
        input: &[u8],
        index: usize,
        count: usize,
    ) -> io::Result<usize> {
        if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
            if self.4 {
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            let mut inner = FsError::new(self.2, FsOperation::Write, "conflicting interruption");
            if !self.5 {
                inner = inner.with_effect_state(self.3);
            }
            let inner = inner
                .with_path(Path::parse("/target").unwrap())
                .with_provider("scripted provider");
            Err(io::Error::new(io::ErrorKind::Interrupted, inner))
        } else {
            let _ = &input[index..index + count];
            Ok(count)
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl FileWriterSpi for Writer {
    fn commit(&mut self) -> Result<WriteOutcome, SpiWriteFailure> {
        self.1.fetch_add(1, Ordering::SeqCst);
        Ok(WriteOutcome::new(
            AchievedAtomicity::Atomic,
            PublicationMethod::AtomicRename,
        ))
    }
    fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        Ok(WriteAbortOutcome::Indeterminate)
    }
}

fn filesystem(
    kind: FsErrorKind,
    effect: FsEffectState,
) -> (FileSystem, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    filesystem_with_ordinary(kind, effect, false)
}

fn filesystem_with_ordinary(
    kind: FsErrorKind,
    effect: FsEffectState,
    ordinary: bool,
) -> (FileSystem, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    filesystem_configured(kind, effect, ordinary, false)
}

fn filesystem_configured(
    kind: FsErrorKind,
    effect: FsEffectState,
    ordinary: bool,
    omit_effect: bool,
) -> (FileSystem, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let commits = Arc::new(AtomicUsize::new(0));
    (
        FileSystem::from_spi(Provider(
            Arc::clone(&calls),
            Arc::clone(&commits),
            kind,
            effect,
            ordinary,
            omit_effect,
        ))
        .unwrap(),
        calls,
        commits,
    )
}

#[test]
fn test_write_fully_does_not_retry_conflicting_interruption() {
    for kind in [
        FsErrorKind::PermissionDenied,
        FsErrorKind::Io,
        FsErrorKind::Indeterminate,
    ] {
        let (fs, calls, commits) = filesystem(kind, FsEffectState::Unchanged);
        let mut writer = fs
            .open_writer(&Path::parse("/target").unwrap(), WriteOptions::default())
            .unwrap();
        let error = writer
            .write_fully(b"payload")
            .expect_err("conflicting error must propagate");
        assert_eq!(1, calls.load(Ordering::SeqCst), "inner kind {kind:?}");
        assert_eq!(io::ErrorKind::Other, error.kind());
        let typed = error
            .get_ref()
            .and_then(|e| e.downcast_ref::<FsError>())
            .expect("typed cause retained");
        assert_eq!(kind, typed.kind());
        assert_eq!(Some("scripted provider"), typed.provider());
        assert_eq!(Some(&Path::parse("/target").unwrap()), typed.path());
        assert_eq!(0, commits.load(Ordering::SeqCst));
        assert_eq!(WriterState::Indeterminate, writer.state());
        assert_eq!(WriteAbortOutcome::Indeterminate, writer.abort().unwrap());
    }
}

#[test]
fn test_typed_interrupted_with_unchanged_is_a_retry_candidate() {
    for effect in [
        FsEffectState::Unchanged,
        FsEffectState::PartiallyApplied,
        FsEffectState::Applied,
        FsEffectState::Indeterminate,
    ] {
        let (fs, calls, _) = filesystem(FsErrorKind::Interrupted, effect);
        let mut writer = fs
            .open_writer(&Path::parse("/target").unwrap(), WriteOptions::default())
            .unwrap();
        let result = writer.write_fully(b"payload");
        if effect == FsEffectState::Unchanged {
            assert!(result.is_ok());
            assert_eq!(2, calls.load(Ordering::SeqCst));
        } else {
            assert!(result.is_err());
            assert_eq!(1, calls.load(Ordering::SeqCst));
        }
    }
}

#[test]
fn test_typed_interrupted_without_effect_is_not_retried() {
    let (fs, calls, _) = filesystem_configured(
        FsErrorKind::Interrupted,
        FsEffectState::Unchanged,
        false,
        true,
    );
    let mut writer = fs
        .open_writer(&Path::parse("/target").unwrap(), WriteOptions::default())
        .unwrap();
    assert!(writer.write_fully(b"payload").is_err());
    assert_eq!(1, calls.load(Ordering::SeqCst));
}

#[test]
fn test_ordinary_interrupted_remains_retryable() {
    let (fs, calls, _) =
        filesystem_with_ordinary(FsErrorKind::Interrupted, FsEffectState::Unchanged, true);
    let mut writer = fs
        .open_writer(&Path::parse("/target").unwrap(), WriteOptions::default())
        .unwrap();
    assert!(writer.write_fully(b"payload").is_ok());
    assert_eq!(2, calls.load(Ordering::SeqCst));
}

#[test]
fn test_stream_copy_does_not_retry_conflicting_interruption() {
    let (fs, calls, commits) = filesystem(FsErrorKind::PermissionDenied, FsEffectState::Unchanged);
    let failure = fs
        .copy(
            &Path::parse("/source").unwrap(),
            &Path::parse("/target").unwrap(),
            CopyOptions::file(),
        )
        .expect_err("stream write failure must be reported");
    assert_eq!(1, calls.load(Ordering::SeqCst));
    assert_eq!(0, commits.load(Ordering::SeqCst));
    assert_eq!(FsErrorKind::PermissionDenied, failure.error().kind());
    assert_eq!(CopyFailureState::Indeterminate, failure.state());
    assert_eq!(FsOperation::Copy, failure.error().operation());
    assert_eq!(Some("scripted provider"), failure.error().provider());
    assert_eq!(
        Some(&Path::parse("/target").unwrap()),
        failure.error().path()
    );
    assert!(
        matches!(failure.recovery(), Some(WriterRecovery::Opened(writer)) if writer.state() == WriterState::Indeterminate)
    );
}
