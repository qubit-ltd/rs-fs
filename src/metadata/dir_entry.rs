// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory entry model.

use crate::{
    FileKind,
    FileMetadata,
    FsPath,
};

/// One entry returned by directory listing.
#[derive(Clone, Debug, PartialEq)]
pub struct DirEntry {
    /// Provider-local path of the entry.
    pub path: FsPath,
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
    #[must_use]
    pub fn new(path: FsPath, kind: FileKind) -> Self {
        let name = path.file_name().unwrap_or("").to_owned();
        Self {
            path,
            name,
            kind,
            metadata: None,
        }
    }
}
