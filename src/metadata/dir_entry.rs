// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Directory entry model.

use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::path::Path;

/// One entry returned by directory listing.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{DirEntry, FileKind};
/// use qubit_fs::path::Path;
///
/// let entry = DirEntry::new(Path::parse("/a")?, FileKind::File);
/// assert_eq!("/a", entry.path.as_str());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DirEntry {
    /// Provider-local path of the entry.
    pub path: Path,
    /// Final path component.
    pub name: String,
    /// Provider-neutral resource kind.
    pub kind: FileKind,
    /// Optional metadata loaded with the entry.
    pub metadata: Option<FileMetadata>,
}

impl DirEntry {
    /// Creates a directory entry.
    ///
    /// # Parameters
    /// - `path`: Provider-local entry path.
    /// - `kind`: Provider-neutral resource kind.
    ///
    /// # Returns
    /// New entry with no loaded metadata.
    #[inline]
    #[must_use]
    pub fn new(path: Path, kind: FileKind) -> Self {
        let name = path.file_name().unwrap_or_default().to_owned();
        Self {
            path,
            name,
            kind,
            metadata: None,
        }
    }
}
