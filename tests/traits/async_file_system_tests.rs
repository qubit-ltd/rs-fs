// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};

use qubit_fs::{
    AsyncFileSystem, CopyOptions, CreateDirOptions, DeleteOptions, FileKind, FileMetadata,
    FileSystemCapabilities, FileSystemCapability, FileSystemId, FileSystemInfo,
    FileSystemProperties, FsError, FsErrorKind, FsFuture, FsOperation, FsPath, ListOptions,
    PathSemantics, ReadOptions, RenameOptions, TempDirOptions, TempFileOptions, WriteOptions,
};

#[derive(Debug)]
struct AsyncStatFs {
    info: FileSystemInfo,
    capabilities: FileSystemCapabilities,
}

impl FileSystemProperties for AsyncStatFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        self.capabilities
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits = qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl AsyncFileSystem for AsyncStatFs {
    fn stat_async<'a>(&'a self, path: &'a FsPath) -> FsFuture<'a, FileMetadata> {
        Box::pin(async move {
            match path.as_str() {
                "/file" => Ok(FileMetadata::new(FileKind::File)),
                "/missing" => Err(FsError::new(
                    FsErrorKind::NotFound,
                    FsOperation::Stat,
                    "missing",
                )),
                _ => Err(FsError::new(
                    FsErrorKind::PermissionDenied,
                    FsOperation::Stat,
                    "denied",
                )),
            }
        })
    }
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
fn async_file_system_is_object_safe_and_uses_suffixed_operations() {
    let fs: Arc<dyn AsyncFileSystem> = Arc::new(AsyncStatFs {
        info: FileSystemInfo::new(
            FileSystemId::new("async-fs").expect("id should parse"),
            "mock",
            PathSemantics::Hierarchical,
        ),
        capabilities: FileSystemCapabilities::default().with(FileSystemCapability::Read),
    });
    let path = FsPath::parse_normalized("/file").expect("path should parse");

    assert_eq!(
        FileKind::File,
        ready(fs.stat_async(&path))
            .expect("stat should succeed")
            .kind,
    );
    assert!(fs.capabilities().contains(FileSystemCapability::Read));
}

#[test]
fn async_file_system_defaults_are_awaitable_capability_failures() {
    let fs = AsyncStatFs {
        info: FileSystemInfo::new(
            FileSystemId::new("async-fs").unwrap(),
            "mock",
            PathSemantics::Hierarchical,
        ),
        capabilities: FileSystemCapabilities::default(),
    };
    let path = FsPath::parse("/file").unwrap();
    let missing = FsPath::parse("/missing").unwrap();
    let denied = FsPath::parse("/denied").unwrap();
    let target = FsPath::parse("/target").unwrap();

    assert!(ready(fs.exists_async(&path)).unwrap());
    assert!(!ready(fs.exists_async(&missing)).unwrap());
    let denied_error = ready(fs.exists_async(&denied)).unwrap_err();
    assert_eq!(FsErrorKind::PermissionDenied, denied_error.kind());
    assert_eq!(FsOperation::Exists, denied_error.operation());

    let rename_error =
        ready(fs.rename_async(&path, &target, RenameOptions::default())).unwrap_err();
    assert_eq!(Some(&path), rename_error.path());
    assert_eq!(Some(&target), rename_error.target());
    let copy_error = ready(fs.copy_async(&path, &target, CopyOptions::default())).unwrap_err();
    assert_eq!(Some(&path), copy_error.path());
    assert_eq!(Some(&target), copy_error.target());

    let failures = [
        ready(fs.list_async(&path, ListOptions::default())).unwrap_err(),
        ready(fs.open_reader_async(&path, ReadOptions::default())).unwrap_err(),
        ready(fs.open_writer_async(&path, WriteOptions::default())).unwrap_err(),
        ready(fs.create_dir_async(&path, CreateDirOptions::default())).unwrap_err(),
        ready(fs.delete_async(&path, DeleteOptions::default())).unwrap_err(),
        rename_error,
        copy_error,
        ready(fs.create_temp_file_async(TempFileOptions::default())).unwrap_err(),
        ready(fs.create_temp_dir_async(TempDirOptions::default())).unwrap_err(),
    ];
    for error in failures {
        assert_eq!(FsErrorKind::UnsupportedCapability, error.kind());
        assert!(error.required_capability().is_some());
        assert_eq!(Some("mock"), error.provider());
    }
}
