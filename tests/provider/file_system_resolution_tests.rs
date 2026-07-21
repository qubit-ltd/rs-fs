// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use qubit_fs::{
    FileSystem,
    FileSystemResolution,
    FsPath,
    FsUri,
};

use crate::common::MockFs;

#[test]
fn resolution_keeps_provider_decoded_path_with_safe_canonical_uri() {
    let fs: Arc<dyn FileSystem> = Arc::new(MockFs::default());
    let resolution = FileSystemResolution::new(
        fs,
        FsPath::parse_literal("bucket/a%252Fb")
            .expect("literal path should parse"),
        FsUri::parse("mock:///bucket/a%25252Fb").expect("URI should parse"),
    );

    assert_eq!("bucket/a%252Fb", resolution.path().as_str());
    assert_eq!(
        "/bucket/a%25252Fb",
        resolution.canonical_uri().path().as_encoded()
    );
    assert_eq!(
        "mock-instance",
        resolution.file_system().info().id().as_str()
    );
    assert!(format!("{resolution:?}").contains("bucket/a%252Fb"));

    let cloned = resolution.clone();
    let (cloned_fs, cloned_path, cloned_uri) = cloned.into_parts();
    assert!(Arc::ptr_eq(resolution.file_system(), &cloned_fs));
    assert_eq!(resolution.path(), &cloned_path);
    assert_eq!(resolution.canonical_uri(), &cloned_uri);
}
