//! Public synchronous random-access contracts and the path-switching model.
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
#[path = "common/random_access.rs"]
mod random_access_common;
use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimit, FileSystemLimits, OpenedFileInfo, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::{ChecksumPolicy, ReadOptions};
use qubit_fs::spi::{
    FileSystemSpi, OpenReaderRequest, OpenedRandomAccessReader, OpenedReader, ProviderOperation,
    ProviderOperations, ProviderProperties, RandomAccessReadSession, StatRequest, StatResponse,
};
use qubit_fs::{FileSystem, FsResult, Path};
use qubit_io::Input;

const ID: &str = "random-access-contract";
type Current = Arc<RwLock<Arc<[u8]>>>;

#[derive(Clone, Copy)]
enum MetadataMode {
    Valid,
    Missing,
    Directory,
    MissingLength,
    WrongIdentity,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum ReadMode {
    Normal,
    Short,
    Overreport,
    Error,
}
struct Provider {
    current: Current,
    random_supported: bool,
    metadata: MetadataMode,
    read_mode: ReadMode,
    limit: FileSystemLimit,
    random_opens: Arc<AtomicUsize>,
    path_opens: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}
struct Session {
    bytes: Arc<[u8]>,
    mode: ReadMode,
    drops: Arc<AtomicUsize>,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl RandomAccessReadSession for Session {
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> FsResult<usize> {
        if self.mode == ReadMode::Error {
            if let Some(first) = out.first_mut() {
                *first = b'P';
            }
            return Err(FsError::with_source(
                FsErrorKind::Io,
                FsOperation::Other,
                "fixture read failed",
                std::io::Error::other("fixture source"),
            ));
        }
        if self.mode == ReadMode::Overreport {
            return Ok(out.len() + 1);
        }
        let start = offset as usize;
        let n = out.len().min(self.bytes.len().saturating_sub(start));
        let n = if self.mode == ReadMode::Short {
            n.min(1)
        } else {
            n
        };
        out[..n].copy_from_slice(&self.bytes[start..start + n]);
        Ok(n)
    }
}
impl Provider {
    fn new() -> Self {
        Self {
            current: Arc::new(RwLock::new(Arc::from(&b"AAAAAA"[..]))),
            random_supported: true,
            metadata: MetadataMode::Valid,
            read_mode: ReadMode::Normal,
            limit: FileSystemLimit::Unknown,
            random_opens: Arc::new(AtomicUsize::new(0)),
            path_opens: Arc::new(AtomicUsize::new(0)),
            drops: Arc::new(AtomicUsize::new(0)),
        }
    }
    fn filesystem(&self) -> FileSystem {
        FileSystem::from_spi(self.clone_for_spi()).unwrap()
    }
    fn clone_for_spi(&self) -> Self {
        Self {
            current: self.current.clone(),
            random_supported: self.random_supported,
            metadata: self.metadata,
            read_mode: self.read_mode,
            limit: self.limit,
            random_opens: self.random_opens.clone(),
            path_opens: self.path_opens.clone(),
            drops: self.drops.clone(),
        }
    }
}
impl FileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        let mut ops = ProviderOperations::new().with(ProviderOperation::OpenReader);
        if self.random_supported {
            ops = ops.with(ProviderOperation::OpenRandomAccessReader);
        }
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new(ID).unwrap(),
                "fixture",
                PathSemantics::Hierarchical,
            ),
            ops,
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown().with_max_read_range_bytes(self.limit),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
    fn open_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedReader> {
        self.path_opens.fetch_add(1, Ordering::SeqCst);
        let bytes = self.current.read().unwrap().to_vec();
        let info = OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), request.path().clone())
            .with_metadata(FileMetadata::new(FileKind::File));
        Ok(OpenedReader::new(info, Box::new(Cursor::new(bytes))))
    }
    fn open_random_access_reader(
        &self,
        request: OpenReaderRequest<'_>,
    ) -> FsResult<OpenedRandomAccessReader> {
        self.random_opens.fetch_add(1, Ordering::SeqCst);
        let bytes = self.current.read().unwrap().clone();
        let path = request.path().clone();
        let info = match self.metadata {
            MetadataMode::Valid => random_access_common::opened_info(ID, &path, bytes.len() as u64),
            MetadataMode::Missing => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path),
            MetadataMode::Directory => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(
                    FileMetadata::new(FileKind::Directory).with_len(Some(bytes.len() as u64)),
                ),
            MetadataMode::MissingLength => {
                OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                    .with_metadata(FileMetadata::new(FileKind::File))
            }
            MetadataMode::WrongIdentity => {
                random_access_common::opened_info("wrong-fs", &path, bytes.len() as u64)
            }
        };
        Ok(OpenedRandomAccessReader::new(
            info,
            Box::new(Session {
                bytes,
                mode: self.read_mode,
                drops: self.drops.clone(),
            }),
        ))
    }
}

#[test]
fn test_path_replacement_splits_legacy_reads_but_session_stays_on_opened_object() {
    let provider = Provider::new();
    let filesystem = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    let mut old_a = filesystem
        .open_reader(&path, ReadOptions::default())
        .unwrap();
    let mut a = [0; 1];
    old_a.read_fully(&mut a).unwrap();
    *provider.current.write().unwrap() = Arc::from(&b"BBBBBB"[..]);
    let mut old_b = filesystem
        .open_reader(&path, ReadOptions::default())
        .unwrap();
    let mut b = [0; 1];
    old_b.read_fully(&mut b).unwrap();
    assert_eq!(*b"A", a);
    assert_eq!(*b"B", b);
    assert_eq!(2, provider.path_opens.load(Ordering::SeqCst));

    *provider.current.write().unwrap() = Arc::from(&b"AAAAAA"[..]);
    let mut reader = filesystem
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    *provider.current.write().unwrap() = Arc::from(&b"BBBBBB"[..]);
    let mut first = [0; 2];
    let mut second = [0; 2];
    assert_eq!(2, reader.read_at(0, &mut first).unwrap());
    assert_eq!(2, reader.read_at(4, &mut second).unwrap());
    assert_eq!(*b"AA", first);
    assert_eq!(*b"AA", second);
    assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
    assert_eq!(2, provider.path_opens.load(Ordering::SeqCst));
    assert_eq!(2, provider.path_opens.load(Ordering::SeqCst));
}

#[test]
fn test_unsupported_operation_and_open_options_are_rejected_before_spi() {
    let mut unsupported = Provider::new();
    unsupported.random_supported = false;
    let unsupported_fs = unsupported.filesystem();
    let path = Path::parse("/same").unwrap();
    let error = unsupported_fs
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(0, unsupported.random_opens.load(Ordering::SeqCst));

    let provider = Provider::new();
    let fs = provider.filesystem();
    let zero_offset = fs
        .open_random_access_reader(&path, ReadOptions::default().with_offset(Some(0)))
        .unwrap();
    assert_eq!(6, zero_offset.len());
    drop(zero_offset);
    assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
    for (options, kind) in [
        (
            ReadOptions::default().with_length(Some(3)),
            FsErrorKind::InvalidOptions,
        ),
        (
            ReadOptions::default().with_checksum(ChecksumPolicy::BestEffort),
            FsErrorKind::RequirementNotMet,
        ),
        (
            ReadOptions::default().with_if_match(Some("v1".into())),
            FsErrorKind::RequirementNotMet,
        ),
    ] {
        let error = fs.open_random_access_reader(&path, options).unwrap_err();
        assert_eq!(kind, error.kind());
        assert_eq!(FsOperation::OpenReader, error.operation());
        assert_eq!(Some(&path), error.path());
        assert_eq!(Some("fixture"), error.provider());
        assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
    }
    let error = fs
        .open_random_access_reader(&Path::parse("relative").unwrap(), ReadOptions::default())
        .unwrap_err();
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some("relative"), error.path().map(Path::as_str));
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
}

#[test]
fn test_invalid_open_envelopes_drop_session_and_preserve_contract_context() {
    for metadata in [
        MetadataMode::Missing,
        MetadataMode::Directory,
        MetadataMode::MissingLength,
        MetadataMode::WrongIdentity,
    ] {
        let mut provider = Provider::new();
        provider.metadata = metadata;
        let error = provider
            .filesystem()
            .open_random_access_reader(&Path::parse("/same").unwrap(), ReadOptions::default())
            .unwrap_err();
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
        let expected_operation = if matches!(metadata, MetadataMode::WrongIdentity) {
            FsOperation::ValidateProviderOutcome
        } else {
            FsOperation::OpenReader
        };
        assert_eq!(expected_operation, error.operation());
        assert_eq!(Some("/same"), error.path().map(Path::as_str));
        assert_eq!(Some("fixture"), error.provider());
        assert_eq!(1, provider.drops.load(Ordering::SeqCst));
    }
}

#[test]
fn test_reads_clip_eof_allow_short_reads_and_reject_overreport_with_context() {
    let mut provider = Provider::new();
    let fs = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    let mut reader = fs
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    let mut tail = [0; 4];
    assert_eq!(2, reader.read_at(4, &mut tail).unwrap());
    assert_eq!(&b"AA"[..], &tail[..2]);
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
    provider.read_mode = ReadMode::Short;
    let mut reader = provider
        .filesystem()
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    let mut short = [0; 3];
    assert_eq!(1, reader.read_at(0, &mut short).unwrap());
    assert_eq!(b'A', short[0]);
    drop(reader);
    provider.read_mode = ReadMode::Overreport;
    let mut reader = provider
        .filesystem()
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    let error = reader.read_at(0, &mut [0; 2]).unwrap_err();
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
}

#[test]
fn test_provider_read_error_keeps_typed_source_and_context() {
    let mut provider = Provider::new();
    provider.read_mode = ReadMode::Error;
    let path = Path::parse("/same").unwrap();
    let mut reader = provider
        .filesystem()
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    let mut bytes = [0; 1];
    let error = reader.read_at(0, &mut bytes).unwrap_err();
    assert_eq!(b'P', bytes[0]);
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(
        Some("fixture source".to_owned()),
        std::error::Error::source(&error).map(ToString::to_string)
    );
}

#[test]
fn test_empty_eof_overflow_limit_and_drop_once() {
    let mut provider = Provider::new();
    provider.limit = FileSystemLimit::Maximum(2);
    let path = Path::parse("/same").unwrap();
    let mut reader = provider
        .filesystem()
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    assert_eq!(0, reader.read_at(0, &mut []).unwrap());
    assert_eq!(0, reader.read_at(6, &mut [0; 1]).unwrap());
    let limit_error = reader.read_at(0, &mut [0; 3]).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, limit_error.kind());
    assert_eq!(FsOperation::Read, limit_error.operation());
    assert_eq!(Some(&path), limit_error.path());
    assert_eq!(Some("fixture"), limit_error.provider());
    let overflow_error = reader.read_at(u64::MAX, &mut [0; 2]).unwrap_err();
    assert_eq!(FsErrorKind::InvalidOptions, overflow_error.kind());
    assert_eq!(FsOperation::Read, overflow_error.operation());
    assert_eq!(Some(&path), overflow_error.path());
    assert_eq!(Some("fixture"), overflow_error.provider());
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
}

struct DefaultSpi;
impl FileSystemSpi for DefaultSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new(ID).unwrap(),
                "fixture",
                PathSemantics::Hierarchical,
            ),
            ProviderOperations::new()
                .with(ProviderOperation::OpenReader)
                .with(ProviderOperation::OpenRandomAccessReader),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat(&self, request: StatRequest<'_>) -> FsResult<StatResponse> {
        Ok(StatResponse::new(
            request.path().clone(),
            FileMetadata::new(FileKind::File),
        ))
    }
}
#[test]
fn test_default_spi_method_returns_unsupported_without_opening() {
    let fs = FileSystem::from_spi(DefaultSpi).unwrap();
    let path = Path::parse("/same").unwrap();
    let error = fs
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
}
