// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Directory entry model.

use crate::{
    FileKind,
    FileMetadata,
    Path,
};

/// One entry returned by directory listing.
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
        let name = path
            .components()
            .last()
            .map(|component| component.to_string())
            .unwrap_or_default();
        Self {
            path,
            name,
            kind,
            metadata: None,
        }
    }
}
