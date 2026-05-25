use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    AtomicityRequirement,
    FileSystem,
    FileSystemExt,
    FsPath,
    ManagedTempFile,
    PersistOptions,
    TempFile,
    TempResource,
};

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_managed_temp_file_cleanup_removes_file() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state));
    let path = FsPath::parse("/manual-file.tmp").expect("path should parse");
    fs.write_all(&path, b"data").expect("write should succeed");

    Box::new(ManagedTempFile::new(fs.clone(), path.clone()))
        .cleanup()
        .expect("cleanup should succeed");

    assert!(!fs.exists(&path).expect("file should be gone"));
}

#[test]
fn test_managed_temp_file_keep_returns_path_without_cleanup() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/manual-file.tmp").expect("path should parse");
    fs.write_all(&path, b"data").expect("write should succeed");

    let kept = Box::new(ManagedTempFile::new(fs, path.clone()))
        .keep()
        .expect("keep should succeed");

    assert_eq!(path, kept);
}

#[test]
fn test_managed_temp_file_persist_renames_when_supported() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/rename-file.tmp").expect("path should parse");
    fs.write_all(&path, b"data").expect("write should succeed");

    Box::new(ManagedTempFile::new(fs.clone(), path))
        .persist(
            &FsPath::parse("/renamed-file.txt").expect("path should parse"),
            &PersistOptions::default(),
        )
        .expect("persist should rename");

    assert!(
        fs.exists(&FsPath::parse("/renamed-file.txt").expect("path should parse"))
            .expect("target should exist"),
    );
}

#[test]
fn test_managed_temp_file_persist_copy_deletes_when_rename_unsupported_and_allowed() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    state.lock().expect("state lock should succeed").fail_rename_unsupported = true;
    let path = FsPath::parse("/copy-file.tmp").expect("path should parse");
    fs.write_all(&path, b"data").expect("write should succeed");

    Box::new(ManagedTempFile::new(fs, path))
        .persist(
            &FsPath::parse("/copy-file.txt").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                atomic: AtomicityRequirement::BestEffort,
                ..PersistOptions::default()
            },
        )
        .expect("persist should copy-delete");
}

#[test]
fn test_managed_temp_file_drop_runs_best_effort_cleanup() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/drop-file.tmp").expect("path should parse");
    fs.write_all(&path, b"data").expect("write should succeed");

    drop(ManagedTempFile::new(fs.clone(), path.clone()));

    assert!(!fs.exists(&path).expect("drop cleanup should run"));
}

#[test]
fn test_managed_temp_file_cleanup_and_persist_return_errors() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    let cleanup_path = FsPath::parse("/cleanup-error.tmp").expect("path should parse");
    fs.write_all(&cleanup_path, b"data").expect("file should be created");
    state.lock().expect("state lock should succeed").fail_delete = true;
    assert!(
        Box::new(ManagedTempFile::new(fs.clone(), cleanup_path))
            .cleanup()
            .is_err(),
    );
    state.lock().expect("state lock should succeed").fail_delete = false;

    state.lock().expect("state lock should succeed").fail_rename_unsupported = true;
    let unsupported_path = FsPath::parse("/unsupported-file.tmp").expect("path should parse");
    fs.write_all(&unsupported_path, b"data")
        .expect("file should be created");
    assert!(
        Box::new(ManagedTempFile::new(fs.clone(), unsupported_path))
            .persist(
                &FsPath::parse("/unsupported-file.txt").expect("path should parse"),
                &PersistOptions::default(),
            )
            .is_err(),
    );
}
