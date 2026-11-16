// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Public lifecycle contract shared by synchronous temporary handles.

mod handle_support;

use qubit_fs::Path;
use qubit_fs::metadata::AtomicityRequirement;
use qubit_fs::temp::PersistOptions;
use qubit_fs::temp::TempOptions;
use qubit_fs::temp::TempResourceState;

#[test]
fn test_file_and_directory_share_successful_lifecycle_contract() {
    let target = Path::parse("/published").expect("target");
    let (filesystem, cleanup_calls, persist_calls) =
        handle_support::implementation_tests::filesystem(false, Vec::new());
    let mut file = filesystem.create_temp_file(TempOptions::default()).expect("file");
    assert!(format!("{file:?}").starts_with("TempFile {"));
    file.persist(
        &target,
        PersistOptions::default().with_atomicity(AtomicityRequirement::Preferred),
    )
    .expect("persist");
    assert_eq!(TempResourceState::Persisted, file.state());
    let file_error = file.cleanup().expect_err("persisted file cannot be cleaned again");
    assert!(
        file_error
            .to_string()
            .contains("temporary file cannot perform this lifecycle operation")
    );
    drop(file);
    assert_eq!(0, *cleanup_calls.lock().expect("cleanup calls"));
    assert_eq!(1, *persist_calls.lock().expect("persist calls"));

    let (filesystem, cleanup_calls, _) = handle_support::implementation_tests::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(TempOptions::default())
        .expect("directory");
    assert!(format!("{directory:?}").starts_with("TempDirectory {"));
    directory.keep().expect("keep");
    assert_eq!(TempResourceState::Kept, directory.state());
    let directory_error = directory.cleanup().expect_err("kept directory cannot be cleaned again");
    assert!(
        directory_error
            .to_string()
            .contains("temporary directory cannot perform this lifecycle operation")
    );
    drop(directory);
    assert_eq!(0, *cleanup_calls.lock().expect("cleanup calls"));
}

#[test]
fn test_file_and_directory_cleanup_and_owned_drop_call_provider_once() {
    let (filesystem, cleanup_calls, _) = handle_support::implementation_tests::filesystem(false, Vec::new());
    let mut file = filesystem.create_temp_file(TempOptions::default()).expect("file");
    file.cleanup().expect("cleanup");
    assert_eq!(TempResourceState::Cleaned, file.state());
    drop(file);
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup calls"));

    let (filesystem, cleanup_calls, _) = handle_support::implementation_tests::filesystem(false, Vec::new());
    let directory = filesystem
        .create_temp_directory(TempOptions::default())
        .expect("directory");
    drop(directory);
    assert_eq!(1, *cleanup_calls.lock().expect("cleanup calls"));
}
