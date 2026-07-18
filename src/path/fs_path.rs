// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-local filesystem path model.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsName,
    FsOperation,
    FsResult,
    RelativeFsPath,
};

/// Provider-local filesystem path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsPath {
    /// Whether the path is absolute.
    absolute: bool,
    /// Provider-local path string using `/` separators.
    path: String,
}

impl FsPath {
    /// Parses a hierarchical filesystem path using normalized semantics.
    ///
    /// # Parameters
    /// - `path`: Raw path string.
    ///
    /// # Returns
    /// Normalized provider-local path.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when the path is empty, contains a NUL byte,
    /// or tries to escape above its root with `..`.
    #[inline]
    pub fn parse(path: &str) -> FsResult<Self> {
        Self::parse_normalized(path)
    }

    /// Parses and normalizes a hierarchical filesystem path.
    ///
    /// Repeated separators and `.` components are removed. `..` components
    /// are resolved and may not escape above the path root.
    ///
    /// # Errors
    ///
    /// Returns an invalid-path error for empty paths, control characters, or
    /// root escape attempts.
    pub fn parse_normalized(path: &str) -> FsResult<Self> {
        if path.is_empty() {
            return Err(crate::FsError::invalid_path(
                FsOperation::ParsePath,
                "path must not be empty",
            ));
        }
        if path.chars().any(char::is_control) {
            return Err(crate::FsError::invalid_path(
                FsOperation::ParsePath,
                "path must not contain control characters",
            ));
        }
        let absolute = path.starts_with('/');
        let mut components = Vec::new();
        for component in path.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(crate::FsError::invalid_path(
                            FsOperation::ParsePath,
                            "path must not escape above its root",
                        ));
                    }
                }
                _ => components.push(component),
            }
        }
        let normalized = if absolute {
            if components.is_empty() {
                "/".to_owned()
            } else {
                format!("/{}", components.join("/"))
            }
        } else {
            components.join("/")
        };
        if normalized.is_empty() {
            return Err(crate::FsError::invalid_path(
                FsOperation::ParsePath,
                "relative path must not normalize to empty",
            ));
        }
        Ok(Self {
            absolute,
            path: normalized,
        })
    }

    /// Parses a provider-literal path without normalizing path components.
    ///
    /// This form is intended for object-key and provider-specific semantics.
    /// It preserves repeated separators, `.`, and `..` as ordinary text.
    ///
    /// # Errors
    ///
    /// Returns an invalid-path error when `path` is empty or contains control
    /// characters.
    pub fn parse_literal(path: &str) -> FsResult<Self> {
        if path.is_empty() {
            return Err(crate::FsError::invalid_path(
                FsOperation::ParsePath,
                "literal path must not be empty",
            ));
        }
        if path.chars().any(char::is_control) {
            return Err(crate::FsError::invalid_path(
                FsOperation::ParsePath,
                "literal path must not contain control characters",
            ));
        }
        Ok(Self {
            absolute: path.starts_with('/'),
            path: path.to_owned(),
        })
    }

    /// Creates the absolute root path.
    ///
    /// # Returns
    /// Root filesystem path.
    #[inline]
    #[must_use]
    pub fn root() -> Self {
        Self {
            absolute: true,
            path: "/".to_owned(),
        }
    }

    /// Tells whether this path is absolute.
    ///
    /// # Returns
    /// `true` when the path starts at the provider root.
    #[inline]
    #[must_use]
    pub fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// Gets the normalized path string.
    ///
    /// # Returns
    /// Normalized path string using `/` separators.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Appends one validated child name.
    #[must_use]
    pub fn child(&self, name: &FsName) -> Self {
        let path = if self.path == "/" {
            format!("/{}", name.as_str())
        } else {
            format!("{}/{}", self.path, name.as_str())
        };
        Self {
            absolute: self.absolute,
            path,
        }
    }

    /// Appends a validated relative descendant path.
    #[must_use]
    pub fn join_relative(&self, relative: &RelativeFsPath) -> Self {
        let path = if self.path == "/" {
            format!("/{}", relative.as_str())
        } else {
            format!("{}/{}", self.path, relative.as_str())
        };
        Self {
            absolute: self.absolute,
            path,
        }
    }

    /// Joins a child path to this path.
    ///
    /// # Parameters
    /// - `child`: Relative descendant path.
    ///
    /// # Returns
    /// Joined descendant path.
    ///
    /// # Errors
    /// Returns [`crate::FsError`] when `child` is empty, absolute, contains a
    /// control character, or escapes above the base with `..`.
    pub fn join(&self, child: &str) -> FsResult<Self> {
        RelativeFsPath::parse(child)
            .map(|relative| self.join_relative(&relative))
    }

    /// Gets this path's parent.
    ///
    /// # Returns
    /// `Some` parent path when the path has one, or `None` for root and
    /// parentless relative paths.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.path == "/" {
            return None;
        }
        let trimmed = self.path.trim_end_matches('/');
        let index = trimmed.rfind('/')?;
        if index == 0 && self.absolute {
            Some(Self::root())
        } else if index == 0 {
            None
        } else {
            Some(Self {
                absolute: self.absolute,
                path: trimmed[..index].to_owned(),
            })
        }
    }

    /// Gets the final path component.
    ///
    /// # Returns
    /// `Some` file name when one exists, or `None` for root.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        if self.path == "/" {
            None
        } else {
            self.path.rsplit('/').next()
        }
    }

    /// Gets the final path component extension.
    ///
    /// # Returns
    /// `Some` extension without the dot when the final path component has a
    /// non-empty extension, or `None` for root, extensionless names, hidden
    /// names such as `.profile`, and names ending with a dot.
    #[must_use]
    pub fn file_extension(&self) -> Option<&str> {
        let file_name = self.file_name()?;
        let index = file_name.rfind('.')?;
        if index == 0 || index + 1 == file_name.len() {
            None
        } else {
            Some(&file_name[index + 1..])
        }
    }
}

impl Display for FsPath {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.path)
    }
}
