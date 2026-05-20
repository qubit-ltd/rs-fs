use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    CreateDirOptions,
    FileSystem,
    FileSystemExt,
    FsPath,
    ManagedTempFile,
    PersistOptions,
    TempDirOptions,
    TempFile,
    TempFileOptions,
    TempResources,
};

use crate::common::{
    MockFs,
    MockState,
    NativeTempFs,
};

#[test]
fn test_temp_resources_create_managed_file_and_dir_with_default_options() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());

    let temp_file = TempResources::create_file(fs.clone(), &TempFileOptions::default())
        .expect("temp file should be created");
    let temp_path = temp_file.path().clone();
    assert!(fs.exists(&temp_path).expect("temp file should exist"));
    temp_file.cleanup().expect("cleanup should succeed");
    assert!(!fs.exists(&temp_path).expect("temp file should be gone"));

    let temp_dir = TempResources::create_dir(fs.clone(), &TempDirOptions::default())
        .expect("temp dir should be created");
    let temp_dir_path = temp_dir.path().clone();
    assert!(fs.exists(&temp_dir_path).expect("temp dir should exist"));
    temp_dir.cleanup().expect("cleanup should succeed");
    assert!(!fs.exists(&temp_dir_path).expect("temp dir should be gone"));
}

#[test]
fn test_temp_resources_convenience_methods_create_default_and_prefixed_resources() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());

    let default_file =
        TempResources::create_default_file(fs.clone()).expect("default temp file should create");
    assert!(default_file.path().as_str().starts_with("/.tmp-"));
    default_file
        .cleanup()
        .expect("default temp file should clean");

    let prefixed_file = TempResources::create_file_with_prefix(fs.clone(), "prefix-")
        .expect("prefixed temp file should create");
    assert!(prefixed_file.path().as_str().starts_with("/prefix-"));
    prefixed_file
        .cleanup()
        .expect("prefixed temp file should clean");

    let default_dir =
        TempResources::create_default_dir(fs.clone()).expect("default temp dir should create");
    assert!(default_dir.path().as_str().starts_with("/.tmp-dir-"));
    default_dir
        .cleanup()
        .expect("default temp dir should clean");

    let prefixed_dir = TempResources::create_dir_with_prefix(fs.clone(), "dir-prefix-")
        .expect("prefixed temp dir should create");
    assert!(prefixed_dir.path().as_str().starts_with("/dir-prefix-"));
    prefixed_dir
        .cleanup()
        .expect("prefixed temp dir should clean");
}

#[test]
fn test_temp_resources_create_custom_paths_with_parent_prefix_and_suffix() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let parent = FsPath::parse("/tmp").expect("path should parse");
    fs.create_dir(&parent, &CreateDirOptions::default())
        .expect("parent should be created");

    let temp_file = TempResources::create_file(
        fs.clone(),
        &TempFileOptions {
            parent: Some(parent.clone()),
            prefix: "pre-".to_owned(),
            suffix: ".tmp".to_owned(),
        },
    )
    .expect("custom temp file should be created");
    assert!(temp_file.path().as_str().starts_with("/tmp/pre-"));
    assert!(temp_file.path().as_str().ends_with(".tmp"));
    temp_file.cleanup().expect("custom temp file should clean");

    let temp_dir = TempResources::create_dir(
        fs.clone(),
        &TempDirOptions {
            parent: Some(parent),
            prefix: "dir-".to_owned(),
            suffix: ".work".to_owned(),
        },
    )
    .expect("custom temp dir should be created");
    assert!(temp_dir.path().as_str().contains("/tmp/dir-"));
    assert!(temp_dir.path().as_str().ends_with(".work"));
    temp_dir.cleanup().expect("custom temp dir should clean");
}

#[test]
fn test_temp_resources_return_errors_from_invalid_paths_and_creation_failures() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    assert!(
        TempResources::create_file(
            fs,
            &TempFileOptions {
                parent: None,
                prefix: "../".to_owned(),
                suffix: String::new(),
            },
        )
        .is_err(),
    );

    let commit_error_state = Arc::new(Mutex::new(MockState {
        fail_commit: true,
        ..MockState::default()
    }));
    let commit_error_fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(commit_error_state));
    assert!(TempResources::create_file(commit_error_fs, &TempFileOptions::default()).is_err());

    let create_dir_error_state = Arc::new(Mutex::new(MockState {
        fail_create_dir: true,
        ..MockState::default()
    }));
    let create_dir_error_fs: Arc<dyn FileSystem> =
        Arc::new(MockFs::with_state(create_dir_error_state));
    assert!(TempResources::create_dir(create_dir_error_fs, &TempDirOptions::default()).is_err());
}

#[test]
fn test_temp_resources_uses_native_temp_factory_when_file_system_provides_one() {
    let native_fs: Arc<dyn FileSystem> = Arc::new(NativeTempFs);
    let native_parent = FsPath::parse("/native").expect("path should parse");

    let native_file = TempResources::create_file(
        native_fs.clone(),
        &TempFileOptions {
            parent: Some(native_parent.clone()),
            prefix: "file-".to_owned(),
            suffix: ".tmp".to_owned(),
        },
    )
    .expect("native temp file should be created");
    assert!(native_file.path().as_str().starts_with("/native/file-"));
    assert!(native_file.path().as_str().ends_with(".tmp"));
    native_file.keep().expect("native temp file should keep");

    let native_dir = TempResources::create_dir(
        native_fs,
        &TempDirOptions {
            parent: Some(native_parent),
            prefix: "dir-".to_owned(),
            suffix: ".work".to_owned(),
        },
    )
    .expect("native temp dir should be created");
    assert!(native_dir.path().as_str().starts_with("/native/dir-"));
    assert!(native_dir.path().as_str().ends_with(".work"));
    native_dir.keep().expect("native temp dir should keep");
}

#[test]
fn test_temp_resources_required_copy_fallback_succeeds_when_allowed() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::with_state(state.clone()));
    state
        .lock()
        .expect("state lock should succeed")
        .fail_rename_unsupported = true;
    let path = FsPath::parse("/required-copy.tmp").expect("path should parse");
    fs.write_all(&path, b"data")
        .expect("file should be created");

    Box::new(ManagedTempFile::new(fs.clone(), path))
        .persist(
            &FsPath::parse("/required-copy.txt").expect("path should parse"),
            &PersistOptions {
                allow_copy_delete: true,
                ..PersistOptions::default()
            },
        )
        .expect("required fallback copy should succeed");
}
