/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! File type model.

/// Provider-neutral resource type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileType {
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
