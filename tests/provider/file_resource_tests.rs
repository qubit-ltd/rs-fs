use std::io::{
    Read,
    Write,
};

use qubit_fs::{
    CopyOptions,
    CreateDirOptions,
    DeleteOptions,
    DirectoryStreamExt,
    FileResource,
    FileSystemRegistry,
    FsPath,
    FsUri,
    ListOptions,
    ReadOptions,
    RenameOptions,
    WriteOptions,
};

use crate::common::{
    MockFs,
    MockProvider,
};

#[test]
fn test_file_resource_delegates_operations_to_resolved_file_system() {
    let fs = MockFs::default();
    let mut registry = FileSystemRegistry::new();
    registry
        .register(MockProvider { fs })
        .expect("provider should register");
    let resource_uri = FsUri::parse("mock:///file.txt").expect("URI should parse");
    let resource = registry
        .resource(&resource_uri)
        .expect("URI should resolve");
    let _: FileResource = resource.clone();

    assert_eq!("/file.txt", resource.path().as_str());
    assert!(resource.fs().capabilities().directories);
    assert!(resource.write_all(b"data").is_ok());
    assert!(resource.exists().expect("resolved fs should work"));
    assert_eq!(
        Some(4),
        resource.metadata().expect("metadata should load").len
    );

    let mut reader = resource
        .open_reader(&ReadOptions::default())
        .expect("reader should open");
    let mut direct_read = Vec::new();
    reader
        .read_to_end(&mut direct_read)
        .expect("reader should read");
    assert_eq!(b"data".to_vec(), direct_read);

    let mut writer = resource
        .open_writer(&WriteOptions::default())
        .expect("writer should open");
    writer.write_all(b"data").expect("writer should write");
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
    let mut registry = FileSystemRegistry::new();
    registry
        .register(MockProvider { fs })
        .expect("provider should register");
    let dir_uri = FsUri::parse("mock:///dir").expect("URI should parse");
    let dir = registry
        .resource(&dir_uri)
        .expect("directory resource should resolve");

    dir.create_dir(&CreateDirOptions::default())
        .expect("resource directory should create");
    assert!(dir.exists().expect("directory should exist"));
    dir.delete(&DeleteOptions::default())
        .expect("resource directory should delete");
}
