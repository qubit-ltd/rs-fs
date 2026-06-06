// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File kind model.

/// Provider-neutral resource kind.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileKind {
    /// Regular file.
    File,
    /// Directory with hierarchical semantics.
    Directory,
    /// Symbolic link.
    Symlink,
    /// Object-store object.
    Object,
    /// Object-store prefix or WebDAV collection-like prefix.
    Prefix,
    /// Provider-specific resource type.
    Other(String),
}
