// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Public synchronous random-access contracts and the path-switching model.
use std::io::Cursor;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
mod common;
use common::random_access as random_access_common;
use qubit_fs::FileSystem;
use qubit_fs::FsResult;
use qubit_fs::Path;
use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemCapabilities;
use qubit_fs::metadata::FileSystemCapability;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::FileSystemInfo;
use qubit_fs::metadata::FileSystemLimit;
use qubit_fs::metadata::FileSystemLimits;
use qubit_fs::metadata::OpenedFileInfo;
use qubit_fs::metadata::SymlinkPolicy;
use qubit_fs::path::PathConstraints;
use qubit_fs::path::PathSemantics;
use qubit_fs::read::ChecksumPolicy;
use qubit_fs::read::ReadOptions;
use qubit_fs::spi::FileSystemSpi;
use qubit_fs::spi::OpenReaderRequest;
use qubit_fs::spi::OpenedRandomAccessReader;
use qubit_fs::spi::OpenedReader;
use qubit_fs::spi::ProviderOperation;
use qubit_fs::spi::ProviderOperations;
use qubit_fs::spi::ProviderProperties;
use qubit_fs::spi::RandomAccessReadSession;
use qubit_fs::spi::StatRequest;
use qubit_fs::spi::StatResponse;
use qubit_io::Input;

const ID: &str = "random-access-contract";
type Current = Arc<RwLock<Arc<[u8]>>>;

#[derive(Clone, Copy)]
enum MetadataMode {
    Valid,
    Missing,
    Directory,
    MissingLength,
    Object,
    ObjectMissingLength,
    Prefix,
    Symlink,
    Other,
    WrongIdentity,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum ReadMode {
    Normal,
    Short,
    Overreport,
    Error,
    ContextError,
}
struct Provider {
    current: Current,
    random_supported: bool,
    metadata: MetadataMode,
    read_mode: ReadMode,
    limit: FileSystemLimit,
    random_opens: Arc<AtomicUsize>,
    path_opens: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    context_path: bool,
    context_provider: bool,
    object_key: bool,
}
struct Session {
    bytes: Arc<[u8]>,
    mode: ReadMode,
    reads: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    context_path: bool,
    context_provider: bool,
}
#[derive(Debug)]
struct SourceMarker;
impl std::fmt::Display for SourceMarker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("context marker")
    }
}
impl std::error::Error for SourceMarker {}
impl Drop for Session {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
impl RandomAccessReadSession for Session {
    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> FsResult<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
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
        if self.mode == ReadMode::ContextError {
            let mut error = FsError::with_source(
                FsErrorKind::Io,
                FsOperation::Other,
                "context fixture failure",
                SourceMarker,
            )
            .with_effect_state(FsEffectState::Indeterminate)
            .with_target(Path::parse("/provider-target").unwrap())
            .with_failure_target(Path::parse("/provider-failure-target").unwrap());
            if self.context_path {
                error = error.with_path(Path::parse("/provider-detail").unwrap());
            }
            if self.context_provider {
                error = error.with_provider("nested-provider");
            }
            return Err(error);
        }
        if self.mode == ReadMode::Overreport {
            return Ok(out.len() + 1);
        }
        let start = offset as usize;
        let n = out.len().min(self.bytes.len().saturating_sub(start));
        let n = if self.mode == ReadMode::Short { n.min(1) } else { n };
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
            reads: Arc::new(AtomicUsize::new(0)),
            drops: Arc::new(AtomicUsize::new(0)),
            context_path: false,
            context_provider: false,
            object_key: false,
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
            reads: self.reads.clone(),
            drops: self.drops.clone(),
            context_path: self.context_path,
            context_provider: self.context_provider,
            object_key: self.object_key,
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
                if self.object_key {
                    PathSemantics::ObjectKey
                } else {
                    PathSemantics::Hierarchical
                },
            ),
            ops,
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::Read),
            FileSystemLimits::unknown().with_max_read_range_bytes(self.limit),
            if self.object_key {
                PathConstraints::relative()
            } else {
                PathConstraints::absolute()
            },
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
    fn open_random_access_reader(&self, request: OpenReaderRequest<'_>) -> FsResult<OpenedRandomAccessReader> {
        self.random_opens.fetch_add(1, Ordering::SeqCst);
        let bytes = self.current.read().unwrap().clone();
        let path = request.path().clone();
        let info = match self.metadata {
            MetadataMode::Valid => random_access_common::opened_info(ID, &path, bytes.len() as u64),
            MetadataMode::Missing => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path),
            MetadataMode::Directory => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Directory).with_len(Some(bytes.len() as u64))),
            MetadataMode::MissingLength => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::File)),
            MetadataMode::Object => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Object).with_len(Some(bytes.len() as u64))),
            MetadataMode::ObjectMissingLength => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Object)),
            MetadataMode::Prefix => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Prefix).with_len(Some(bytes.len() as u64))),
            MetadataMode::Symlink => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Symlink).with_len(Some(bytes.len() as u64))),
            MetadataMode::Other => OpenedFileInfo::new(FileSystemId::new(ID).unwrap(), path)
                .with_metadata(FileMetadata::new(FileKind::Other("other".into())).with_len(Some(bytes.len() as u64))),
            MetadataMode::WrongIdentity => random_access_common::opened_info("wrong-fs", &path, bytes.len() as u64),
        };
        Ok(OpenedRandomAccessReader::new(
            info,
            Box::new(Session {
                bytes,
                mode: self.read_mode,
                reads: self.reads.clone(),
                drops: self.drops.clone(),
                context_path: self.context_path,
                context_provider: self.context_provider,
            }),
        ))
    }
}

#[test]
fn test_path_replacement_splits_legacy_reads_but_session_stays_on_opened_object() {
    let provider = Provider::new();
    let filesystem = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    let mut old_a = filesystem.open_reader(&path, ReadOptions::default()).unwrap();
    let mut a = [0; 1];
    old_a.read_fully(&mut a).unwrap();
    *provider.current.write().unwrap() = Arc::from(&b"BBBBBB"[..]);
    let mut old_b = filesystem.open_reader(&path, ReadOptions::default()).unwrap();
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
        (ReadOptions::default().with_length(Some(3)), FsErrorKind::InvalidOptions),
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
        MetadataMode::ObjectMissingLength,
        MetadataMode::Prefix,
        MetadataMode::Symlink,
        MetadataMode::Other,
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
fn test_object_key_random_access_uses_known_length_and_stays_on_opened_object() {
    let mut provider = Provider::new();
    provider.object_key = true;
    provider.metadata = MetadataMode::Object;
    *provider.current.write().unwrap() = Arc::from(&b"object-data"[..]);
    let filesystem = provider.filesystem();
    let path = Path::parse_with_semantics("bucket//raw/%2Fobject", PathSemantics::ObjectKey).unwrap();
    let mut reader = filesystem
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    assert_eq!(11, reader.len());
    assert_eq!(&FileSystemId::new(ID).unwrap(), reader.info().filesystem_id());
    assert_eq!(&path, reader.info().path());
    assert_eq!(
        Some(&FileKind::Object),
        reader.info().metadata().map(FileMetadata::kind)
    );

    *provider.current.write().unwrap() = Arc::from(&b"replacement"[..]);
    let mut first = [0; 6];
    let mut second = [0; 4];
    assert_eq!(6, reader.read_at(0, &mut first).unwrap());
    assert_eq!(4, reader.read_at(7, &mut second).unwrap());
    assert_eq!(*b"object", first);
    assert_eq!(*b"data", second);
    assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
    assert_eq!(0, provider.path_opens.load(Ordering::SeqCst));
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
}

#[test]
fn test_zero_length_object_returns_zero_without_provider_read() {
    let mut provider = Provider::new();
    provider.metadata = MetadataMode::Object;
    *provider.current.write().unwrap() = Arc::from(&b""[..]);
    let path = Path::parse("/empty-object").unwrap();
    let mut reader = provider
        .filesystem()
        .open_random_access_reader(&path, ReadOptions::default())
        .unwrap();
    assert_eq!(0, reader.len());
    assert_eq!(0, reader.read_at(0, &mut [0; 1]).unwrap());
    assert_eq!(0, provider.reads.load(Ordering::SeqCst));
    assert_eq!(1, provider.random_opens.load(Ordering::SeqCst));
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
}

#[test]
fn test_reads_clip_eof_allow_short_reads_and_reject_overreport_with_context() {
    let mut provider = Provider::new();
    let fs = provider.filesystem();
    let path = Path::parse("/same").unwrap();
    let mut reader = fs.open_random_access_reader(&path, ReadOptions::default()).unwrap();
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
fn test_provider_error_context_is_preserved_or_filled_independently() {
    let session_path = Path::parse("/same").unwrap();
    let provider_path = Path::parse("/provider-detail").unwrap();
    let target = Path::parse("/provider-target").unwrap();
    let failure_target = Path::parse("/provider-failure-target").unwrap();
    for (has_path, has_provider) in [(true, true), (false, false), (true, false), (false, true)] {
        let mut provider = Provider::new();
        provider.read_mode = ReadMode::ContextError;
        provider.context_path = has_path;
        provider.context_provider = has_provider;
        let mut reader = provider
            .filesystem()
            .open_random_access_reader(&session_path, ReadOptions::default())
            .unwrap();
        let error = reader.read_at(0, &mut [0; 1]).unwrap_err();
        assert_eq!(
            Some(if has_path { &provider_path } else { &session_path }),
            error.path()
        );
        assert_eq!(
            Some(if has_provider { "nested-provider" } else { "fixture" }),
            error.provider()
        );
        assert_eq!(FsOperation::Read, error.operation());
        assert_eq!(FsErrorKind::Io, error.kind());
        assert_eq!(Some(FsEffectState::Indeterminate), error.effect_state());
        assert_eq!(Some(&target), error.target());
        assert_eq!(Some(&failure_target), error.failure_target());
        assert!(std::error::Error::source(&error).unwrap().is::<SourceMarker>());
        assert_eq!(1, provider.reads.load(Ordering::SeqCst));
    }
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
    assert_eq!(0, provider.reads.load(Ordering::SeqCst));
    let limit_error = reader.read_at(0, &mut [0; 3]).unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, limit_error.kind());
    assert_eq!(FsOperation::Read, limit_error.operation());
    assert_eq!(Some(&path), limit_error.path());
    assert_eq!(Some("fixture"), limit_error.provider());
    assert_eq!(0, provider.reads.load(Ordering::SeqCst));
    let overflow_error = reader.read_at(u64::MAX, &mut [0; 2]).unwrap_err();
    assert_eq!(FsErrorKind::InvalidOptions, overflow_error.kind());
    assert_eq!(FsOperation::Read, overflow_error.operation());
    assert_eq!(Some(&path), overflow_error.path());
    assert_eq!(Some("fixture"), overflow_error.provider());
    assert_eq!(0, provider.reads.load(Ordering::SeqCst));
    drop(reader);
    assert_eq!(1, provider.drops.load(Ordering::SeqCst));
}

struct DefaultSpi;
impl FileSystemSpi for DefaultSpi {
    fn properties(&self) -> ProviderProperties {
        ProviderProperties::new(
            FileSystemInfo::new(FileSystemId::new(ID).unwrap(), "fixture", PathSemantics::Hierarchical),
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
    let error = fs.open_random_access_reader(&path, ReadOptions::default()).unwrap_err();
    assert_eq!(FsErrorKind::UnsupportedOperation, error.kind());
    assert_eq!(FsOperation::OpenReader, error.operation());
    assert_eq!(Some(&path), error.path());
    assert_eq!(Some("fixture"), error.provider());
}
