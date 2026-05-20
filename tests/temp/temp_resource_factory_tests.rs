use qubit_fs::{
    FsPath,
    TempResourceFactory,
};

use crate::common::NativeTempResourceFactory;

#[test]
fn test_make_temp_path_uses_parent_prefix_and_suffix() {
    let parent = FsPath::parse("/tmp").expect("path should parse");
    let path = NativeTempResourceFactory
        .make_temp_path(Some(&parent), "pre-", ".tmp")
        .expect("temp path should be created");

    assert!(path.as_str().starts_with("/tmp/pre-"));
    assert!(path.as_str().ends_with(".tmp"));
}

#[test]
fn test_make_temp_path_rejects_invalid_prefix() {
    assert!(
        NativeTempResourceFactory
            .make_temp_path(None, "../", "")
            .is_err()
    );
}
