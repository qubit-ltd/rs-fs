use std::io::{
    Read,
    Write,
};
use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    CreateDirOptions,
    DirectoryStreamExt,
    FileSystem,
    FileSystemExt,
    FsPath,
    ListOptions,
    ManagedTempFile,
    ReadOptions,
    TempDirOptions,
    TempFileOptions,
    TempResource,
    TempResources,
    WriteOptions,
};

use crate::common::{
    MockFs,
    MockState,
    NativeTempFs,
};

#[test]
fn test_temp_resource_default_methods_delegate_to_file_system() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/resource.tmp").expect("path should parse");
    fs.write_all(&path, b"data")
        .expect("file should be created");
    let temp = ManagedTempFile::new(fs.clone(), path.clone());

    assert!(temp.fs().capabilities().directories);
    assert_eq!(path, temp.path().clone());
    assert!(temp.exists().expect("temporary file should exist"));
    assert_eq!(
        Some(4),
        temp.metadata().expect("metadata should be available").len,
    );

    let resource = temp.resource();
    assert_eq!(path, resource.path().clone());
    assert!(resource.exists().expect("resource should exist"));
}

#[test]
fn test_temp_file_default_io_methods_delegate_to_file_system() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let temp = TempResources::create_file(fs, &TempFileOptions::default())
        .expect("temporary file should be created");

    let mut writer = temp
        .open_writer(&WriteOptions::default())
        .expect("temporary writer should open");
    writer.write_all(b"data").expect("writer should write");
    writer.commit().expect("writer should commit");

    let mut reader = temp
        .open_reader(&ReadOptions::default())
        .expect("temporary reader should open");
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).expect("reader should read");
    assert_eq!(b"data".to_vec(), bytes);

    temp.write_all(b"data")
        .expect("temporary file should write all");
    assert_eq!(
        b"data".to_vec(),
        temp.read_all().expect("temporary file should read all"),
    );
}

#[test]
fn test_temp_dir_default_directory_methods_delegate_to_file_system() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let temp = TempResources::create_dir(fs, &TempDirOptions::default())
        .expect("temporary directory should be created");

    let child = temp
        .child("child.txt")
        .expect("child resource should be created");
    assert!(child.path().as_str().ends_with("/child.txt"));
    child
        .write_all(b"data")
        .expect("child file should be writable");
    assert!(child.exists().expect("child file should exist"));

    let child_dir = temp
        .create_child_dir("child-dir", &CreateDirOptions::default())
        .expect("child directory should be created");
    assert!(child_dir.exists().expect("child directory should exist"));

    let entries = temp
        .list(&ListOptions::default())
        .expect("temporary directory list should start")
        .collect_entries()
        .expect("temporary directory list should collect");
    assert_eq!(1, entries.len());
}

#[test]
fn test_temp_dir_child_rejects_invalid_child_path() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let temp = TempResources::create_dir(fs, &TempDirOptions::default())
        .expect("temporary directory should be created");

    assert!(temp.child("../escape").is_err());
    assert!(
        temp.create_child_dir("../escape", &CreateDirOptions::default())
            .is_err(),
    );
}

#[test]
fn test_temp_dir_create_child_dir_returns_create_errors() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    let temp = TempResources::create_dir(fs, &TempDirOptions::default())
        .expect("temporary directory should be created");
    state
        .lock()
        .expect("state lock should succeed")
        .fail_create_dir = true;

    assert!(
        temp.create_child_dir("child-dir", &CreateDirOptions::default())
            .is_err(),
    );
}

#[test]
fn test_temp_dir_list_returns_file_system_errors() {
    let native_fs: Arc<dyn FileSystem> = Arc::new(NativeTempFs);
    let temp = TempResources::create_dir(native_fs, &TempDirOptions::default())
        .expect("temporary directory should be created");

    assert!(temp.list(&ListOptions::default()).is_err());
}
