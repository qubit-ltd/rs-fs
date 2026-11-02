//! Runtime-neutral async random-access contracts, including cancellation.
use std::future::Future;
use std::future::poll_fn;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Waker};
#[path = "common/random_access.rs"]
mod random_access_common;
use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
use qubit_fs::metadata::{
    FileKind, FileMetadata, FileSystemCapabilities, FileSystemCapability, FileSystemId,
    FileSystemInfo, FileSystemLimits, OpenedFileInfo, SymlinkPolicy,
};
use qubit_fs::path::{PathConstraints, PathSemantics};
use qubit_fs::read::{ChecksumPolicy, ReadOptions};
use qubit_fs::spi::{
    AsyncFileSystemSpi, AsyncRandomAccessReadSession, OpenReaderRequest,
    OpenedAsyncRandomAccessReader, ProviderOperation, ProviderOperations, ProviderProperties,
    SpiFuture, StatRequest, StatResponse,
};
use qubit_fs::{AsyncFileSystem, FsResult, Path};
const ID: &str = "async-random-access-contract";
#[derive(Clone, Copy, Eq, PartialEq)]
enum ReadMode {
    Normal,
    Short,
    Overreport,
    Error,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum MetadataMode {
    Valid,
    Missing,
    Directory,
    MissingLength,
    WrongIdentity,
}
struct Provider {
    opens: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    mode: ReadMode,
    metadata: MetadataMode,
    supported: bool,
    pending_first: bool,
}
struct Session {
    reads: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    mode: ReadMode,
    pending_first: bool,
}
impl Drop for Session {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl AsyncRandomAccessReadSession for Session {
    fn read_at_async<'a>(
        self: Pin<&'a mut Self>,
        offset: u64,
        out: &'a mut [u8],
    ) -> SpiFuture<'a, FsResult<usize>> {
        Box::pin(async move {
            let this = self.get_mut();
            let call = this.reads.fetch_add(1, Ordering::SeqCst);
            if this.pending_first && call == 0 {
                if let Some(first) = out.first_mut() {
                    *first = b'P';
                }
                poll_fn(|_| Poll::<()>::Pending).await;
            }
            if this.mode == ReadMode::Error {
                if let Some(first) = out.first_mut() {
                    *first = b'E';
                }
                return Err(FsError::with_source(
                    FsErrorKind::Io,
                    FsOperation::Other,
                    "fixture async read failed",
                    std::io::Error::other("async source"),
                ));
            }
            if this.mode == ReadMode::Overreport {
                return Ok(out.len() + 1);
            }
            let bytes = b"AAAAAA";
            let start = offset as usize;
            let mut count = out.len().min(bytes.len().saturating_sub(start));
            if this.mode == ReadMode::Short {
                count = count.min(1);
            }
            out[..count].copy_from_slice(&bytes[start..start + count]);
            Ok(count)
        })
    }
}
impl Provider {
    fn new() -> Self {
        Self {
            opens: Arc::new(AtomicUsize::new(0)),
            reads: Arc::new(AtomicUsize::new(0)),
            drops: Arc::new(AtomicUsize::new(0)),
            mode: ReadMode::Normal,
            metadata: MetadataMode::Valid,
            supported: true,
            pending_first: false,
        }
    }
    fn filesystem(&self) -> AsyncFileSystem {
        AsyncFileSystem::from_spi(Self {
            opens: self.opens.clone(),
            reads: self.reads.clone(),
            drops: self.drops.clone(),
            mode: self.mode,
            metadata: self.metadata,
            supported: self.supported,
            pending_first: self.pending_first,
        })
        .unwrap()
    }
}
impl AsyncFileSystemSpi for Provider {
    fn properties(&self) -> ProviderProperties {
        let mut ops = ProviderOperations::new().with(ProviderOperation::OpenReader);
        if self.supported {
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
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .unwrap()
    }
    fn stat<'a>(&'a self, r: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            Ok(StatResponse::new(
                r.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        })
    }
    fn open_random_access_reader<'a>(
        &'a self,
        r: OpenReaderRequest<'a>,
    ) -> SpiFuture<'a, FsResult<OpenedAsyncRandomAccessReader>> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let reads = self.reads.clone();
        let drops = self.drops.clone();
        let mode = self.mode;
        let metadata = self.metadata;
        let pending_first = self.pending_first;
        Box::pin(async move {
            let path = r.path().clone();
            let info = match metadata {
                MetadataMode::Valid => random_access_common::opened_info(ID, &path, 6),
                MetadataMode::Missing => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path),
                MetadataMode::Directory => {
                    OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                        .with_metadata(FileMetadata::new(FileKind::Directory).with_len(Some(6)))
                }
                MetadataMode::MissingLength => {
                    OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                        .with_metadata(FileMetadata::new(FileKind::File))
                }
                MetadataMode::WrongIdentity => {
                    random_access_common::opened_info("wrong-id", &path, 6)
                }
            };
            Ok(OpenedAsyncRandomAccessReader::new(
                info,
                Box::new(Session {
                    reads,
                    drops,
                    mode,
                    pending_first,
                }),
            ))
        })
    }
}
fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("expected a ready future"),
    }
}

#[test]
fn test_async_unpolled_and_cancelled_reads_leave_session_reusable() {
    let mut provider = Provider::new();
    provider.pending_first = true;
    let fs = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    let mut reader = ready(fs.open_random_access_reader(&path, ReadOptions::default())).unwrap();
    let mut out = [0xCC; 2];
    {
        let unpolled = reader.read_at(4, &mut out);
        drop(unpolled);
    }
    assert_eq!(0, provider.reads.load(Ordering::SeqCst));
    assert_eq!([0xCC; 2], out);
    {
        let mut pending = std::pin::pin!(reader.read_at(4, &mut out));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(pending.as_mut().poll(&mut cx).is_pending());
    }
    assert_eq!(1, provider.reads.load(Ordering::SeqCst));
    assert_eq!([b'P', 0xCC], out);
    assert_eq!(2, ready(reader.read_at(0, &mut out)).unwrap());
    assert_eq!(*b"AA", out);
    assert_eq!(2, provider.reads.load(Ordering::SeqCst));
    assert_eq!(1, provider.opens.load(Ordering::SeqCst));
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
}

#[test]
fn test_async_open_preflight_matches_sync_error_classification() {
    let provider = Provider::new();
    let fs = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    for (options, kind) in [
        (
            ReadOptions::default().with_length(Some(2)),
            FsErrorKind::InvalidOptions,
        ),
        (
            ReadOptions::default().with_checksum(ChecksumPolicy::BestEffort),
            FsErrorKind::RequirementNotMet,
        ),
        (
            ReadOptions::default().with_if_match(Some("version".into())),
            FsErrorKind::RequirementNotMet,
        ),
    ] {
        let error = ready(fs.open_random_access_reader(&path, options)).unwrap_err();
        assert_eq!(kind, error.kind());
        assert_eq!(FsOperation::OpenReader, error.operation());
        assert_eq!(Some(&path), error.path());
        assert_eq!(Some("fixture"), error.provider());
        assert_eq!(0, provider.opens.load(Ordering::SeqCst));
    }
    let error = ready(
        fs.open_random_access_reader(&Path::parse("relative").unwrap(), ReadOptions::default()),
    )
    .unwrap_err();
    assert_eq!(FsErrorKind::InvalidPath, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some("relative"), error.path().map(Path::as_str));
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(0, provider.opens.load(Ordering::SeqCst));
    let zero_offset =
        ready(fs.open_random_access_reader(&path, ReadOptions::default().with_offset(Some(0))))
            .unwrap();
    assert_eq!(6, zero_offset.len());
    drop(zero_offset);
    assert_eq!(1, provider.opens.load(Ordering::SeqCst));
}

#[test]
fn test_async_envelope_contract_errors_release_session() {
    for metadata in [
        MetadataMode::Missing,
        MetadataMode::Directory,
        MetadataMode::MissingLength,
        MetadataMode::WrongIdentity,
    ] {
        let mut provider = Provider::new();
        provider.metadata = metadata;
        let path = Path::parse("/same").unwrap();
        let error = ready(
            provider
                .filesystem()
                .open_random_access_reader(&path, ReadOptions::default()),
        )
        .unwrap_err();
        assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
        let expected_operation = if matches!(metadata, MetadataMode::WrongIdentity) {
            FsOperation::ValidateProviderOutcome
        } else {
            FsOperation::OpenReader
        };
        assert_eq!(expected_operation, error.operation());
        assert_eq!(Some(&path), error.path());
        assert_eq!(Some("fixture"), error.provider());
        assert_eq!(1, provider.drops.load(Ordering::SeqCst));
    }
}

#[test]
fn test_async_short_eof_overreport_and_typed_error() {
    let path = Path::parse("/same").unwrap();
    let mut provider = Provider::new();
    let mut reader = ready(
        provider
            .filesystem()
            .open_random_access_reader(&path, ReadOptions::default()),
    )
    .unwrap();
    let mut tail = [0; 4];
    assert_eq!(2, ready(reader.read_at(4, &mut tail)).unwrap());
    assert_eq!(&b"AA"[..], &tail[..2]);
    drop(reader);
    provider.mode = ReadMode::Short;
    let mut reader = ready(
        provider
            .filesystem()
            .open_random_access_reader(&path, ReadOptions::default()),
    )
    .unwrap();
    assert_eq!(1, ready(reader.read_at(0, &mut [0; 2])).unwrap());
    drop(reader);
    provider.mode = ReadMode::Overreport;
    let mut reader = ready(
        provider
            .filesystem()
            .open_random_access_reader(&path, ReadOptions::default()),
    )
    .unwrap();
    let error = ready(reader.read_at(0, &mut [0; 2])).unwrap_err();
    assert_eq!(FsErrorKind::ProviderContractViolation, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
    drop(reader);
    provider.mode = ReadMode::Error;
    let mut reader = ready(
        provider
            .filesystem()
            .open_random_access_reader(&path, ReadOptions::default()),
    )
    .unwrap();
    let mut bytes = [0; 1];
    let error = ready(reader.read_at(0, &mut bytes)).unwrap_err();
    assert_eq!(b'E', bytes[0]);
    assert_eq!(FsErrorKind::Io, error.kind());
    assert_eq!(FsOperation::Read, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(
        Some("async source".to_owned()),
        std::error::Error::source(&error).map(ToString::to_string)
    );
}

#[test]
fn test_async_operation_undeclared_is_rejected_without_spi_call() {
    let mut provider = Provider::new();
    provider.supported = false;
    let path = Path::parse("/same").unwrap();
    let error = ready(
        provider
            .filesystem()
            .open_random_access_reader(&path, ReadOptions::default()),
    )
    .unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
    assert_eq!(0, provider.opens.load(Ordering::SeqCst));
}

struct DefaultAsyncProvider;
impl AsyncFileSystemSpi for DefaultAsyncProvider {
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
    fn stat<'a>(&'a self, request: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
        Box::pin(async move {
            Ok(StatResponse::new(
                request.path().clone(),
                FileMetadata::new(FileKind::File),
            ))
        })
    }
}
#[test]
fn test_async_default_spi_method_returns_unsupported() {
    let fs = AsyncFileSystem::from_spi(DefaultAsyncProvider).unwrap();
    let path = Path::parse("/same").unwrap();
    let error = ready(fs.open_random_access_reader(&path, ReadOptions::default())).unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
}
