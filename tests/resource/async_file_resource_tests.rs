// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AchievedAtomicity,
    AsyncDirectoryStream,
    AsyncDirectoryStreamSession,
    AsyncFileReader,
    AsyncFileResource,
    AsyncFileSystem,
    AsyncFileWriteSession,
    AsyncFileWriter,
    AtomicityRequirement,
    CopyMethod,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    CreateDirOptions,
    DeleteOptions,
    DirEntry,
    FileKind,
    FileLocation,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemLimit,
    FileSystemLimits,
    FileSystemProperties,
    FsFuture,
    FsPath,
    ListOptions,
    OpenedFileInfo,
    PathSemantics,
    PublicationMethod,
    ReadOptions,
    RenameOptions,
    RenameOutcome,
    ServerSidePreference,
    WriteOptions,
    WriteOutcome,
};
use qubit_io::{
    AsyncInput,
    AsyncOutput,
};
use qubit_spi::ProviderId;

struct ResourceAsyncFs {
    info: FileSystemInfo,
    limits: FileSystemLimits,
    expected_list_page_size: Option<Option<usize>>,
}

impl ResourceAsyncFs {
    fn new() -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new("async-resource").unwrap(),
                ProviderId::new("async-resource").unwrap(),
                PathSemantics::Hierarchical,
            ),
            limits: FileSystemLimits::unknown(),
            expected_list_page_size: None,
        }
    }

    fn with_limits(mut self, limits: FileSystemLimits) -> Self {
        self.limits = limits;
        self
    }

    fn expecting_list_page_size(mut self, page_size: Option<usize>) -> Self {
        self.expected_list_page_size = Some(page_size);
        self
    }
}

impl FileSystemProperties for ResourceAsyncFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
            .with(FileSystemCapability::Read)
            .with(FileSystemCapability::Write)
    }

    fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

impl AsyncFileSystem for ResourceAsyncFs {
    fn stat_async<'a>(
        &'a self,
        _path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata> {
        Box::pin(async { Ok(FileMetadata::new(FileKind::File)) })
    }

    fn list_async<'a>(
        &'a self,
        path: &'a FsPath,
        options: ListOptions,
    ) -> FsFuture<'a, AsyncDirectoryStream> {
        if let Some(expected) = self.expected_list_page_size {
            assert_eq!(expected, options.page_size);
        }
        let entry = DirEntry::new(path.clone(), FileKind::File);
        Box::pin(async move {
            Ok(AsyncDirectoryStream::new(ResourceDirectorySession {
                entry: Some(entry),
            }))
        })
    }

    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader> {
        let info = opened_info(path);
        Box::pin(async move {
            Ok(AsyncFileReader::new(
                ResourceInput {
                    bytes: b"data".to_vec(),
                    position: 0,
                },
                info,
            ))
        })
    }

    fn open_writer_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: WriteOptions,
    ) -> FsFuture<'a, AsyncFileWriter> {
        let info = opened_info(path);
        Box::pin(
            async move { Ok(AsyncFileWriter::new(ResourceWriteSession, info)) },
        )
    }

    fn create_dir_async<'a>(
        &'a self,
        _path: &'a FsPath,
        _options: CreateDirOptions,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn delete_async<'a>(
        &'a self,
        _path: &'a FsPath,
        _options: DeleteOptions,
    ) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }

    fn rename_async<'a>(
        &'a self,
        _from: &'a FsPath,
        _to: &'a FsPath,
        _options: RenameOptions,
    ) -> FsFuture<'a, RenameOutcome> {
        Box::pin(async {
            Ok(RenameOutcome::new(
                AchievedAtomicity::Atomic,
                PublicationMethod::AtomicRename,
            ))
        })
    }

    fn copy_async<'a>(
        &'a self,
        _from: &'a FsPath,
        _to: &'a FsPath,
        _options: CopyOptions,
    ) -> FsFuture<'a, CopyOutcome> {
        Box::pin(async {
            Ok(CopyOutcome::new(
                CopyStats::default(),
                CopyMethod::Stream,
                AchievedAtomicity::NonAtomic,
            ))
        })
    }
}

struct ResourceDirectorySession {
    entry: Option<DirEntry>,
}

impl AsyncDirectoryStreamSession for ResourceDirectorySession {
    fn next_entry_async(&mut self) -> FsFuture<'_, Option<DirEntry>> {
        let entry = self.entry.take();
        Box::pin(async move { Ok(entry) })
    }
}

struct ResourceInput {
    bytes: Vec<u8>,
    position: usize,
}

impl AsyncInput for ResourceInput {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        output: &mut [u8],
        index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        let this = self.get_mut();
        let read = count.min(this.bytes.len().saturating_sub(this.position));
        output[index..index + read]
            .copy_from_slice(&this.bytes[this.position..this.position + read]);
        this.position += read;
        Poll::Ready(Ok(read))
    }
}

struct ResourceWriteSession;

impl AsyncOutput for ResourceWriteSession {
    type Item = u8;

    unsafe fn poll_write_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _input: &[u8],
        _index: usize,
        count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(count))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncFileWriteSession for ResourceWriteSession {
    fn commit_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, WriteOutcome> {
        Box::pin(async {
            Ok(WriteOutcome::new(
                AchievedAtomicity::Atomic,
                PublicationMethod::Direct,
            ))
        })
    }

    fn abort_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

fn opened_info(path: &FsPath) -> OpenedFileInfo {
    OpenedFileInfo::new(FileLocation::new(
        FileSystemId::new("async-resource").unwrap(),
        path.clone(),
    ))
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

#[test]
fn async_file_resource_delegates_every_operation_and_binds_handles() {
    let fs: Arc<dyn AsyncFileSystem> = Arc::new(ResourceAsyncFs::new());
    let path = FsPath::parse("/file").unwrap();
    let location = FileLocation::new(fs.info().id().clone(), path);
    let resource = AsyncFileResource::from_location(fs, location);
    let target = FsPath::parse("/target").unwrap();

    assert_eq!("/file", resource.path().as_str());
    assert_eq!(
        "async-resource",
        resource.location().file_system_id().as_str()
    );
    assert!(
        resource
            .fs()
            .capabilities()
            .contains(FileSystemCapability::Read),
    );
    assert!(format!("{resource:?}").contains("AsyncFileResource"));
    assert_eq!(FileKind::File, ready(resource.stat_async()).unwrap().kind);
    assert!(ready(resource.exists_async()).unwrap());
    assert_eq!(
        b"data",
        ready(resource.read_all_async(4)).unwrap().as_slice()
    );
    let error = ready(resource.read_all_async(3)).unwrap_err();
    assert_eq!(qubit_fs::FsErrorKind::ResourceLimitExceeded, error.kind());
    assert_eq!(qubit_fs::FsOperation::Read, error.operation());
    let outcome = ready(resource.write_all_async(b"updated"))
        .expect("write-all should succeed");
    assert_eq!(PublicationMethod::Direct, outcome.method);

    let mut stream =
        ready(resource.list_async(ListOptions::default())).unwrap();
    assert!(ready(stream.next_entry_async()).unwrap().is_some());

    let reader = ready(resource.open_reader_async(ReadOptions::default()))
        .expect("reader should open");
    assert_eq!(resource.location(), reader.info().location());

    let mut writer = ready(resource.open_writer_async(WriteOptions::default()))
        .expect("writer should open");
    assert_eq!(resource.location(), writer.info().location());
    ready(writer.abort_async()).expect("writer should abort");

    ready(resource.create_dir_async(CreateDirOptions::default())).unwrap();
    ready(resource.delete_async(DeleteOptions::default())).unwrap();
    ready(resource.rename_to_async(&target, RenameOptions::default())).unwrap();
    ready(resource.copy_to_async(&target, CopyOptions::default())).unwrap();

    let read = ReadOptions {
        offset: Some(1),
        ..ReadOptions::default()
    };
    assert!(ready(resource.open_reader_async(read)).is_err());
    let write = WriteOptions {
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    assert!(ready(resource.open_writer_async(write)).is_err());
    let delete = DeleteOptions {
        recursive: true,
        ..DeleteOptions::default()
    };
    assert!(ready(resource.delete_async(delete)).is_err());
    let rename = RenameOptions {
        atomicity: AtomicityRequirement::Required,
        ..RenameOptions::default()
    };
    assert!(ready(resource.rename_to_async(&target, rename)).is_err());
    let copy = CopyOptions {
        server_side: ServerSidePreference::Require,
        ..CopyOptions::default()
    };
    assert!(ready(resource.copy_to_async(&target, copy)).is_err());
}

#[test]
fn async_file_resource_preflights_provider_path_and_range_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(4))
        .with_max_read_range_bytes(FileSystemLimit::Maximum(4));
    let fs: Arc<dyn AsyncFileSystem> =
        Arc::new(ResourceAsyncFs::new().with_limits(limits));
    let resource = AsyncFileResource::new(fs, FsPath::parse("/file").unwrap());

    let path_error = ready(resource.stat_async()).unwrap_err();
    assert_eq!(
        qubit_fs::FsErrorKind::ResourceLimitExceeded,
        path_error.kind(),
    );
    assert_eq!(qubit_fs::FsOperation::Stat, path_error.operation());

    let fs: Arc<dyn AsyncFileSystem> =
        Arc::new(ResourceAsyncFs::new().with_limits(limits));
    let short_resource =
        AsyncFileResource::new(fs, FsPath::parse("/a").unwrap());
    let range_error = ready(short_resource.open_reader_async(ReadOptions {
        length: Some(5),
        ..ReadOptions::default()
    }))
    .unwrap_err();
    assert_eq!(
        qubit_fs::FsErrorKind::ResourceLimitExceeded,
        range_error.kind(),
    );
    assert_eq!(qubit_fs::FsOperation::OpenReader, range_error.operation());
}

#[test]
fn async_file_resource_preflights_paths_for_every_bound_operation() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(3));
    let long = FsPath::parse("/long").unwrap();
    let short = FsPath::parse("/a").unwrap();
    let long_resource = AsyncFileResource::new(
        Arc::new(ResourceAsyncFs::new().with_limits(limits)),
        long.clone(),
    );

    assert!(ready(long_resource.exists_async()).is_err());
    assert!(ready(long_resource.list_async(ListOptions::default())).is_err());
    assert!(
        ready(long_resource.open_reader_async(ReadOptions::default())).is_err()
    );
    assert!(
        ready(long_resource.open_writer_async(WriteOptions::default()))
            .is_err()
    );
    assert!(
        ready(long_resource.create_dir_async(CreateDirOptions::default()))
            .is_err()
    );
    assert!(
        ready(long_resource.delete_async(DeleteOptions::default())).is_err()
    );
    assert!(
        ready(long_resource.rename_to_async(&short, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(long_resource.copy_to_async(&short, CopyOptions::default()))
            .is_err()
    );

    let short_resource = AsyncFileResource::new(
        Arc::new(ResourceAsyncFs::new().with_limits(limits)),
        short,
    );
    assert!(
        ready(short_resource.rename_to_async(&long, RenameOptions::default()))
            .is_err()
    );
    assert!(
        ready(short_resource.copy_to_async(&long, CopyOptions::default()))
            .is_err()
    );
}

#[test]
fn async_file_resource_clamps_list_page_size_before_delegation() {
    let limits = FileSystemLimits::unknown()
        .with_max_list_page_entries(FileSystemLimit::Maximum(64));
    let fs: Arc<dyn AsyncFileSystem> = Arc::new(
        ResourceAsyncFs::new()
            .with_limits(limits)
            .expecting_list_page_size(Some(64)),
    );
    let resource = AsyncFileResource::new(fs, FsPath::parse("/dir").unwrap());

    ready(resource.list_async(ListOptions {
        page_size: Some(100),
        ..ListOptions::default()
    }))
    .unwrap();
}
