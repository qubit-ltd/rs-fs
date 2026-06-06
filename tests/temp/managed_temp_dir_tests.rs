use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    AtomicityRequirement,
    CreateDirOptions,
    FileSystem,
    FsPath,
    ManagedTempDir,
    PersistOptions,
    TempDir,
    TempResource,
};

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_managed_temp_dir_cleanup_removes_directory() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/manual-dir.tmp").expect("path should parse");
    fs.create_dir(&path, &CreateDirOptions::default())
        .expect("dir should be created");

    Box::new(ManagedTempDir::new(fs.clone(), path.clone()))
        .cleanup()
        .expect("cleanup should succeed");

    assert!(!fs.exists(&path).expect("dir should be gone"));
}

#[test]
fn test_managed_temp_dir_keep_returns_path_without_cleanup() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/manual-dir.tmp").expect("path should parse");
    fs.create_dir(&path, &CreateDirOptions::default())
        .expect("dir should be created");

    let kept = Box::new(ManagedTempDir::new(fs, path.clone()))
        .keep()
        .expect("keep should succeed");

    assert_eq!(path, kept);
}

#[test]
fn test_managed_temp_dir_persist_renames_when_supported() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/rename-dir.tmp").expect("path should parse");
    fs.create_dir(&path, &CreateDirOptions::default())
        .expect("dir should be created");

    Box::new(ManagedTempDir::new(fs.clone(), path))
        .persist(
            &FsPath::parse("/renamed-dir").expect("path should parse"),
            &PersistOptions::default(),
        )
        .expect("persist should rename");

    assert!(
        fs.exists(&FsPath::parse("/renamed-dir").expect("path should parse"))
            .expect("target should exist"),
    );
}

#[test]
fn test_managed_temp_dir_persist_copy_deletes_when_rename_unsupported_and_allowed()
 {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    state
        .lock()
        .expect("state lock should succeed")
        .fail_rename_unsupported = true;
    let path = FsPath::parse("/copy-dir.tmp").expect("path should parse");
    fs.create_dir(&path, &CreateDirOptions::default())
        .expect("dir should be created");

    Box::new(ManagedTempDir::new(fs, path))
        .persist(
            &FsPath::parse("/copy-dir").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                atomic: AtomicityRequirement::BestEffort,
                ..PersistOptions::default()
            },
        )
        .expect("persist should copy-delete");
}

#[test]
fn test_managed_temp_dir_drop_runs_best_effort_cleanup() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let path = FsPath::parse("/drop-dir.tmp").expect("path should parse");
    fs.create_dir(&path, &CreateDirOptions::default())
        .expect("dir should be created");

    drop(ManagedTempDir::new(fs.clone(), path.clone()));

    assert!(!fs.exists(&path).expect("drop cleanup should run"));
}

#[test]
fn test_managed_temp_dir_cleanup_and_persist_return_errors() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    let cleanup_path =
        FsPath::parse("/cleanup-dir-error.tmp").expect("path should parse");
    fs.create_dir(&cleanup_path, &CreateDirOptions::default())
        .expect("dir should be created");
    state.lock().expect("state lock should succeed").fail_delete = true;
    assert!(
        Box::new(ManagedTempDir::new(fs.clone(), cleanup_path))
            .cleanup()
            .is_err(),
    );
    state.lock().expect("state lock should succeed").fail_delete = false;

    state
        .lock()
        .expect("state lock should succeed")
        .fail_rename_unsupported = true;
    let unsupported_path =
        FsPath::parse("/unsupported-dir.tmp").expect("path should parse");
    fs.create_dir(&unsupported_path, &CreateDirOptions::default())
        .expect("dir should be created");
    assert!(
        Box::new(ManagedTempDir::new(fs.clone(), unsupported_path))
            .persist(
                &FsPath::parse("/unsupported-dir").expect("path should parse"),
                &PersistOptions::default(),
            )
            .is_err(),
    );
}
