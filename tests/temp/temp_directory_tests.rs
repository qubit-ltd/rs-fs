// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[test]
fn test_temp_directory_child_components_are_lexically_safe() {
    let (filesystem, cleanup_calls, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut directory = filesystem
        .create_temp_directory(qubit_fs::TempDirectoryOptions::default())
        .expect("temporary directory should open");
    let component = qubit_fs::PathComponent::parse("child")
        .expect("component should parse");
    assert_eq!("/temporary/child", directory.child(&component).as_str());
    directory.cleanup().expect("cleanup should succeed");
    assert_eq!(qubit_fs::TempResourceState::Cleaned, directory.state());
    assert_eq!(
        1,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}
