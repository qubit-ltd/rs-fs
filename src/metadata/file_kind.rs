// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! File kind model.

/// Provider-neutral resource kind.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::FileKind;
///
/// let kind = FileKind::Other("archive".to_owned());
/// match kind {
///     FileKind::Other(name) => assert_eq!(name, "archive"),
///     _ => unreachable!("expected the provider-specific kind"),
/// }
/// ```
#[must_use = "inspect or apply the resource classification"]
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
    Other(
        /// Provider-defined resource kind name.
        String,
    ),
}
