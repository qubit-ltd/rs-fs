// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated relative descendant path.

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

use super::native_path_text::validate_canonical_text;

/// A normalized non-empty relative path that cannot escape its base.
///
/// Every component follows the canonical native-path text rules documented by
/// [`crate::NativePathCodec`] and [`crate::FsPath`]. In particular, `%XX` is
/// native-byte escaping, not URI percent encoding, and non-canonical aliases
/// are rejected before relative-path normalization.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RelativeFsPath(Box<str>);

impl RelativeFsPath {
    /// Parses and normalizes a safe relative path.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `path` is empty, absolute, has malformed or
    /// non-canonical native-path escaping, contains a literal control
    /// character, or attempts to escape above its relative root.
    pub fn parse(path: &str) -> FsResult<Self> {
        validate_relative_path_text(path)?;
        if path.is_empty() {
            return Err(invalid_relative("relative path must not be empty"));
        }
        if path.starts_with('/') {
            return Err(invalid_relative("relative path must not be absolute"));
        }
        if path.chars().any(char::is_control) {
            return Err(invalid_relative(
                "relative path must not contain control characters",
            ));
        }
        let mut components = Vec::new();
        for component in path.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(invalid_relative(
                            "relative path must not escape its base",
                        ));
                    }
                }
                _ => components.push(component),
            }
        }
        if components.is_empty() {
            return Err(invalid_relative(
                "relative path must identify a descendant",
            ));
        }
        Ok(Self(components.join("/").into()))
    }

    /// Returns the normalized relative path text.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validates the shared canonical native-path text invariant.
///
/// # Errors
///
/// Returns an invalid-path error when `path` has malformed or non-canonical
/// native-path escaping.
fn validate_relative_path_text(path: &str) -> FsResult<()> {
    validate_canonical_text(path).map_err(|_| {
        FsError::invalid_path(
            FsOperation::ParsePath,
            "relative path text must use canonical native-path escaping",
        )
    })
}

impl Display for RelativeFsPath {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

/// Creates an invalid-path error for a rejected relative path.
fn invalid_relative(message: &str) -> FsError {
    FsError::invalid_path(FsOperation::ParsePath, message)
}
