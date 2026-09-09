// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit resource-prefix and configured-namespace listing scopes.

use crate::path::Path;

/// Selects the portion of one configured filesystem to enumerate.
///
/// A flat [`Path`](Self::Path) scope is a raw key prefix. A hierarchical path
/// denotes a directory. [`Namespace`](Self::Namespace) is available only for
/// flat path semantics and never expands beyond the configured filesystem.
///
/// # Examples
///
/// ```
/// use qubit_fs::Path;
/// use qubit_fs::directory::ListScope;
/// let scope = ListScope::Path(Path::parse_literal("reports/")?);
/// assert_eq!(scope.path().map(Path::as_str), Some("reports/"));
/// assert!(ListScope::Namespace.path().is_none());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ListScope {
    /// Enumerates a directory or raw key prefix using its existing path rules.
    Path(Path),
    /// Enumerates the entire configured flat namespace without an empty path.
    Namespace,
}

impl ListScope {
    /// Returns the resource prefix, or `None` for a whole namespace query.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            Self::Namespace => None,
        }
    }
}
