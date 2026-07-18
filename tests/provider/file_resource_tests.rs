// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use qubit_io::{
    Input,
    Output,
};

use qubit_fs::{
    AtomicityRequirement,
    CopyOptions,
    CreateDirOptions,
    DeleteOptions,
    DirectoryStreamExt,
    FileResource,
    FileSystem,
    FileSystemCapabilities,
    FileSystemCapability,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FileSystemRegistry,
    FsPath,
    FsResult,
    FsUri,
    ListOptions,
    ReadOptions,
    RenameOptions,
    ServerSidePreference,
    WriteOptions,
};
use qubit_spi::ProviderId;

use crate::common::{
    MockFs,
    MockProvider,
};

#[test]
fn test_file_resource_delegates_operations_to_resolved_file_system() {
    let fs = MockFs::default();
    let registry = registry_with_mock(MockProvider {
        descriptor: mock_descriptor(),
        fs,
    });
    let resource_uri =
        FsUri::parse("mock:///file.txt").expect("URI should parse");
    let resource = registry
        .resource_uri(&resource_uri)
        .expect("URI should resolve");
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
        .open_reader(&ReadOptions::default())
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
        .open_writer(&WriteOptions::default())
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
        resource.read_all().expect("resource should read")
    );

    let entries = resource
        .list(&ListOptions::default())
        .expect("resource list should start")
        .collect_entries()
        .expect("resource list should collect");
    assert_eq!(1, entries.len());

    let copied = resource
        .copy_to(
            &FsPath::parse("/copy.txt").expect("path should parse"),
            &CopyOptions::file(),
        )
        .expect("resource should copy");
    assert_eq!(1, copied.stats.files);

    resource
        .rename_to(
            &FsPath::parse("/renamed.txt").expect("path should parse"),
            &RenameOptions::default(),
        )
        .expect("resource should rename");
}

#[test]
fn test_file_resource_delegates_directory_create_and_delete() {
    let fs = MockFs::default();
    let registry = registry_with_mock(MockProvider {
        descriptor: mock_descriptor(),
        fs,
    });
    let dir_uri = FsUri::parse("mock:///dir").expect("URI should parse");
    let dir = registry
        .resource_uri(&dir_uri)
        .expect("directory resource should resolve");

    dir.create_dir(&CreateDirOptions::default())
        .expect("resource directory should create");
    assert!(dir.exists().expect("directory should exist"));
    dir.delete(&DeleteOptions::default())
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
            ProviderId::new("no-capabilities").unwrap(),
            qubit_fs::PathSemantics::Hierarchical,
        ),
    });
    let resource = FileResource::new(fs, FsPath::parse("/file").unwrap());
    let target = FsPath::parse("/target").unwrap();

    let read = ReadOptions {
        offset: Some(1),
        ..ReadOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.open_reader(&read).unwrap_err().kind(),
    );

    let write = WriteOptions {
        atomicity: AtomicityRequirement::Required,
        ..WriteOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.open_writer(&write).unwrap_err().kind(),
    );

    let delete = DeleteOptions {
        recursive: true,
        ..DeleteOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.delete(&delete).unwrap_err().kind(),
    );

    let rename = RenameOptions {
        atomicity: AtomicityRequirement::Required,
        ..RenameOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.rename_to(&target, &rename).unwrap_err().kind(),
    );

    let copy = CopyOptions {
        server_side: ServerSidePreference::Require,
        ..CopyOptions::default()
    };
    assert_eq!(
        qubit_fs::FsErrorKind::RequirementNotMet,
        resource.copy_to(&target, &copy).unwrap_err().kind(),
    );
}

fn registry_with_mock(provider: MockProvider) -> FileSystemRegistry {
    let registry = FileSystemRegistry::default();
    registry
        .register(provider)
        .expect("provider should register");
    registry
}

fn mock_descriptor() -> qubit_spi::ProviderDescriptor {
    qubit_spi::ProviderDescriptor::new(
        qubit_spi::ProviderId::new("mock").expect("valid provider ID"),
    )
    .with_aliases(["mem"])
    .expect("valid aliases")
}
