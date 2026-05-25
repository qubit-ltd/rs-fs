use std::sync::{
    Arc,
    Mutex,
};

use qubit_fs::{
    FileSystemExt,
    FsPath,
};

use crate::common::{
    MockFs,
    MockState,
};

#[test]
fn test_read_all_and_write_all_success_paths() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state);
    let path = FsPath::parse("/file.txt").expect("path should parse");

    fs.write_all(&path, b"data").expect("write_all should succeed");
    assert_eq!(b"data".to_vec(), fs.read_all(&path).expect("read_all should succeed"),);
}

#[test]
fn test_read_all_and_write_all_error_branches() {
    let state = Arc::new(Mutex::new(MockState::default()));
    let fs = MockFs::with_state(state.clone());
    let path = FsPath::parse("/file.txt").expect("path should parse");

    state.lock().expect("state lock should succeed").fail_read = true;
    assert!(fs.read_all(&path).is_err());
    state.lock().expect("state lock should succeed").fail_read = false;

    state.lock().expect("state lock should succeed").fail_write = true;
    assert!(fs.write_all(&path, b"data").is_err());
    assert_eq!(1, state.lock().expect("state lock should succeed").aborts);
}
