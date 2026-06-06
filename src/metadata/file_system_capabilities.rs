// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem capability declaration.

/// Static capability hints exposed by one filesystem implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FileSystemCapabilities {
    /// Whether the backend has hierarchical path semantics.
    pub hierarchical_paths: bool,
    /// Whether the backend supports directories.
    pub directories: bool,
    /// Whether the backend can represent empty directories.
    pub empty_directories: bool,
    /// Whether the backend supports symbolic links.
    pub symlinks: bool,
    /// Whether range reads are supported.
    pub range_read: bool,
    /// Whether append writes are supported.
    pub append: bool,
    /// Whether random writes are supported.
    pub random_write: bool,
    /// Whether rename can be atomic.
    pub atomic_rename: bool,
    /// Whether replacement writes can be atomic.
    pub atomic_replace: bool,
    /// Whether conditional write operations are supported.
    pub conditional_write: bool,
    /// Whether server-side copy is supported.
    pub server_side_copy: bool,
    /// Whether recursive delete is supported.
    pub recursive_delete: bool,
    /// Whether temporary files are supported.
    pub temp_file: bool,
    /// Whether temporary directories are supported.
    pub temp_dir: bool,
    /// Whether temporary resource persistence is supported.
    pub temp_persist: bool,
    /// Whether temporary resource persistence can be atomic.
    pub temp_persist_atomic: bool,
    /// Whether provider-native metadata is exposed.
    pub native_metadata: bool,
}
