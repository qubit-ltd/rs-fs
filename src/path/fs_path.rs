/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Provider-local filesystem path model.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsError,
    FsOperation,
    FsResult,
};

/// Provider-local filesystem path.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsPath {
    /// Whether the path is absolute.
    absolute: bool,
    /// Normalized path string using `/` separators.
    normalized: String,
}

impl FsPath {
    /// Parses and normalizes a filesystem path.
    ///
    /// # Parameters
    /// - `path`: Raw path string.
    ///
    /// # Returns
    /// Normalized provider-local path.
    ///
    /// # Errors
    /// Returns [`FsError`] when the path is empty, contains a NUL byte, or tries
    /// to escape above its root with `..`.
    pub fn parse(path: &str) -> FsResult<Self> {
        if path.is_empty() {
            return Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "path must not be empty",
            ));
        }
        if path.contains('\0') {
            return Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "path must not contain NUL bytes",
            ));
        }
        let absolute = path.starts_with('/');
        let mut components = Vec::new();
        for component in path.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(FsError::invalid_path(
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
            return Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "relative path must not normalize to empty",
            ));
        }
        Ok(Self {
            absolute,
            normalized,
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
            normalized: "/".to_owned(),
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
        &self.normalized
    }

    /// Joins a child path to this path.
    ///
    /// # Parameters
    /// - `child`: Relative or absolute child path.
    ///
    /// # Returns
    /// Joined path. Absolute child paths replace the base path.
    ///
    /// # Errors
    /// Returns [`FsError`] when `child` is not a valid path.
    pub fn join(&self, child: &str) -> FsResult<Self> {
        let child = Self::parse(child)?;
        if child.is_absolute() {
            return Ok(child);
        }
        let joined = if self.normalized == "/" {
            format!("/{}", child.as_str())
        } else {
            format!("{}/{}", self.normalized, child.as_str())
        };
        Self::parse(&joined)
    }

    /// Gets this path's parent.
    ///
    /// # Returns
    /// `Some` parent path when the path has one, or `None` for root and
    /// parentless relative paths.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.normalized == "/" {
            return None;
        }
        let trimmed = self.normalized.trim_end_matches('/');
        let index = trimmed.rfind('/')?;
        if index == 0 && self.absolute {
            Some(Self::root())
        } else if index == 0 {
            None
        } else {
            Self::parse(&trimmed[..index]).ok()
        }
    }

    /// Gets the final path component.
    ///
    /// # Returns
    /// `Some` file name when one exists, or `None` for root.
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        if self.normalized == "/" {
            None
        } else {
            self.normalized.rsplit('/').next()
        }
    }
}

impl Display for FsPath {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(&self.normalized)
    }
}
