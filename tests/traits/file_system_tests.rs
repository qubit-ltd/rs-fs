use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    FileSystem,
    FileSystemExt,
    FsPath,
};

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_file_system_capabilities_exist_and_path_metadata_work() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state);
    let path = FsPath::parse("/file.txt").expect("path should parse");

    assert!(fs.capabilities().directories);
    assert!(!fs.exists(&path).expect("exists should succeed"));
    fs.write_all(&path, b"data")
        .expect("write_all should succeed");
    assert!(fs.exists(&path).expect("exists should succeed"));
    assert_eq!(
        4,
        fs.path_metadata(&path)
            .expect("metadata should exist")
            .len
            .expect("file length should be set"),
    );
}
