// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use qubit_io::{Input, Output};

use qubit_fs::{
    AtomicityRequirement, CopyOptions, CreateDirOptions, DeleteOptions, DirectoryStream,
    DirectoryStreamExt, FileResource, FileSystem, FileSystemCapabilities, FileSystemCapability,
    FileSystemId, FileSystemInfo, FileSystemLimit, FileSystemLimits, FileSystemProperties, FsError,
    FsErrorKind, FsOperation, FsPath, FsResult, FsUri, ListOptions, PathSemantics, ReadOptions,
    RenameOptions, ServerSidePreference, WriteOptions,
};

use crate::common::MockFs;

#[test]
fn test_file_resource_delegates_operations_to_resolved_file_system() {
    let fs = MockFs::default();
    let resource_uri = FsUri::parse("mock:///file.txt").expect("URI should parse");
    let resource = resource_with_mock(fs, "/file.txt", resource_uri.clone());
    let _: FileResource = resource.clone();

    assert_eq!("/file.txt", resource.path().as_str());
    assert_eq!(Some(&resource_uri), resource.location().uri());
    assert!(format!("{resource:?}").contains("FileResource"));
    assert!(
        resource
            .fs()
            .capabilities()
            .contains(FileSystemCapability::CreateDirectory)
    );
    assert!(resource.write_all(b"data").is_ok());
    assert!(resource.exists().expect("resolved fs should work"));
    assert_eq!(Some(4), resource.stat().expect("metadata should load").len);

    let mut reader = resource
        .open_reader(ReadOptions::default())
        .expect("reader should open");
    assert_eq!(
        Some(&resource_uri),
        reader.info().location().uri(),
        "the resolved canonical URI should be fixed on the opened handle",
    );
    let mut direct_read = [0_u8; 4];
    reader
        .read_fully(&mut direct_read)
        .expect("reader should read");
    assert_eq!(b"data", &direct_read);

    let mut writer = resource
        .open_writer(WriteOptions::default())
        .expect("writer should open");
    assert_eq!(
        Some(&resource_uri),
        writer.info().location().uri(),
        "the resolved canonical URI should be fixed on the opened handle",
    );
    writer.write_fully(b"data").expect("writer should write");
    assert_eq!(
        Some(4),
        writer.commit().expect("writer should commit").bytes_written,
    );
    assert_eq!(
        b"data".to_vec(),
        resource.read_all(4).expect("resource should read")
    );

    let entries = resource
        .list(ListOptions::default())
        .expect("resource list should start")
        .collect_entries(1)
        .expect("resource list should collect");
    assert_eq!(1, entries.len());

    let copied = resource
        .copy_to(
            &FsPath::parse("/copy.txt").expect("path should parse"),
            CopyOptions::file(),
        )
        .expect("resource should copy");
    assert_eq!(1, copied.stats.files);

    resource
        .rename_to(
            &FsPath::parse("/renamed.txt").expect("path should parse"),
            RenameOptions::default(),
        )
        .expect("resource should rename");
}

#[test]
fn test_file_resource_delegates_directory_create_and_delete() {
    let fs = MockFs::default();
    let dir_uri = FsUri::parse("mock:///dir").expect("URI should parse");
    let dir = resource_with_mock(fs, "/dir", dir_uri);

    dir.create_dir(CreateDirOptions::default())
        .expect("resource directory should create");
    assert!(dir.exists().expect("directory should exist"));
    dir.delete(DeleteOptions::default())
        .expect("resource directory should delete");
}

struct NoCapabilitiesFs {
    info: FileSystemInfo,
}

impl FileSystemProperties for NoCapabilitiesFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }

    fn limits(&self) -> &qubit_fs::FileSystemLimits {
        static LIMITS: qubit_fs::FileSystemLimits = qubit_fs::FileSystemLimits::unknown();
        &LIMITS
    }
}

impl FileSystem for NoCapabilitiesFs {
    fn stat(&self, _path: &FsPath) -> FsResult<qubit_fs::FileMetadata> {
        Ok(qubit_fs::FileMetadata::new(qubit_fs::FileKind::File))
    }
}

#[test]
fn file_resource_rejects_unmet_requirements_before_delegation() {
    let fs: Arc<dyn FileSystem> = Arc::new(NoCapabilitiesFs {
        info: FileSystemInfo::new(
            FileSystemId::new("no-capabilities").unwrap(),
            "no-capabilities",
            qubit_fs::PathSemantics::Hierarchical,
        ),
    });
    let resource = FileResource::new(fs, FsPath::parse("/file").unwrap());
    let target = FsPath::parse("/target").unwrap();

    let read = ReadOptions {
        offset: Some(1),
        ..ReadOptions::default()
    };
    let read_error = resource.open_reader(read).unwrap_err();
    assert_eq!(qubit_fs::FsErrorKind::RequirementNotMet, read_error.kind());
    assert_eq!(Some("/file"), read_error.path().map(FsPath::as_str));
    assert_eq!(Some("no-capabilities"), read_error.provider());

    let write = WriteOptions {
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.open_writer(write).unwrap_err().kind(),
    );

    let delete = DeleteOptions {
        recursive: true,
        ..DeleteOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.delete(delete).unwrap_err().kind(),
    );

    let rename = RenameOptions {
        atomicity: AtomicityRequirement::Required,
        ..RenameOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.rename_to(&target, rename).unwrap_err().kind(),
    );

    let copy = CopyOptions {
        server_side: ServerSidePreference::Require,
        ..CopyOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.copy_to(&target, copy).unwrap_err().kind(),
    );
}

#[test]
fn file_resource_preflights_provider_path_and_range_limits() {
    let limits = FileSystemLimits::unknown()
        .with_max_path_text_bytes(FileSystemLimit::Maximum(8))
        .with_max_read_range_bytes(FileSystemLimit::Maximum(4));
    let fs = MockFs::default().with_limits(limits);
    let resource = resource_with_mock(
        fs.clone(),
        "/file.txt",
        FsUri::parse("mock:///file.txt").unwrap(),
    );

    let path_error = resource.stat().unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, path_error.kind());
    assert_eq!(qubit_fs::FsOperation::Stat, path_error.operation());

    let short_resource = FileResource::new(Arc::new(fs), FsPath::parse("/a").unwrap());
    let range_error = short_resource
        .open_reader(ReadOptions {
            length: Some(5),
            ..ReadOptions::default()
        })
        .unwrap_err();
    assert_eq!(FsErrorKind::ResourceLimitExceeded, range_error.kind());
    assert_eq!(qubit_fs::FsOperation::OpenReader, range_error.operation());
}

#[test]
fn file_resource_preflights_paths_for_every_bound_operation() {
    let limits = FileSystemLimits::unknown().with_max_path_text_bytes(FileSystemLimit::Maximum(3));
    let long = FsPath::parse("/long").unwrap();
    let short = FsPath::parse("/a").unwrap();
    let long_resource = FileResource::new(
        Arc::new(MockFs::default().with_limits(limits)),
        long.clone(),
    );

    assert!(long_resource.exists().is_err());
    assert!(long_resource.list(ListOptions::default()).is_err());
    assert!(long_resource.open_reader(ReadOptions::default()).is_err());
    assert!(long_resource.open_writer(WriteOptions::default()).is_err());
    assert!(
        long_resource
            .create_dir(CreateDirOptions::default())
            .is_err()
    );
    assert!(long_resource.delete(DeleteOptions::default()).is_err());
    assert!(
        long_resource
            .rename_to(&short, RenameOptions::default())
            .is_err()
    );
    assert!(
        long_resource
            .copy_to(&short, CopyOptions::default())
            .is_err()
    );

    let short_resource = FileResource::new(
        Arc::new(MockFs::default().with_limits(limits)),
        short.clone(),
    );
    let rename_error = short_resource
        .rename_to(&long, RenameOptions::default())
        .unwrap_err();
    assert_eq!(Some(&short), rename_error.path());
    assert_eq!(Some(&long), rename_error.target());
    let copy_error = short_resource
        .copy_to(&long, CopyOptions::default())
        .unwrap_err();
    assert_eq!(Some(&short), copy_error.path());
    assert_eq!(Some(&long), copy_error.target());
}

struct ListPageLimitFs {
    info: FileSystemInfo,
    limits: FileSystemLimits,
}

impl FileSystemProperties for ListPageLimitFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default().with(FileSystemCapability::List)
    }

    fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }
}

impl FileSystem for ListPageLimitFs {
    fn stat(&self, _path: &FsPath) -> FsResult<qubit_fs::FileMetadata> {
        unreachable!("stat is not used by this test")
    }

    fn list(&self, path: &FsPath, options: ListOptions) -> FsResult<DirectoryStream> {
        assert_eq!(Some(64), options.page_size);
        Err(FsError::new(
            FsErrorKind::Other,
            FsOperation::List,
            "observed clamped page size",
        )
        .with_path(path.clone()))
    }
}

#[test]
fn file_resource_clamps_list_page_size_before_delegation() {
    let fs: Arc<dyn FileSystem> = Arc::new(ListPageLimitFs {
        info: FileSystemInfo::new(
            FileSystemId::new("list-page-limit").unwrap(),
            "mock",
            PathSemantics::Hierarchical,
        ),
        limits: FileSystemLimits::unknown()
            .with_max_list_page_entries(FileSystemLimit::Maximum(64)),
    });
    let resource = FileResource::new(fs, FsPath::parse("/dir").unwrap());

    let result = resource.list(ListOptions {
        page_size: Some(100),
        ..ListOptions::default()
    });

    assert!(result.is_err());
}

fn resource_with_mock(fs: MockFs, path: &str, uri: FsUri) -> FileResource {
    let fs: Arc<dyn FileSystem> = Arc::new(fs);
    let path = FsPath::parse(path).expect("path should parse");
    FileResource::from_resolved(fs, path, uri)
}
