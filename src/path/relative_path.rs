// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Safe normalized relative logical paths.

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

/// A non-empty normalized relative path that cannot escape its base.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RelativePath(
    /// Normalized descendant path text.
    String,
);

impl RelativePath {
    /// Parses a normalized relative path.
    ///
    /// Returns an invalid-path error for empty or absolute input, NUL, or a
    /// traversal sequence that escapes above the relative root.
    pub fn parse(text: &str) -> FsResult<Self> {
        if text.is_empty() || text.starts_with('/') || text.contains('\0') {
            return Err(invalid_relative());
        }
        let mut components = Vec::new();
        for component in text.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(invalid_relative());
                    }
                }
                value => components.push(value),
            }
        }
        if components.is_empty() {
            return Err(invalid_relative());
        }
        Ok(Self(components.join("/")))
    }

    /// Returns the normalized logical path text.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RelativePath {
    /// Formats the normalized relative path.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

/// Builds the shared relative-path validation failure.
fn invalid_relative() -> FsError {
    FsError::invalid_path(
        FsOperation::ParsePath,
        "relative path must identify a descendant without escaping its base",
    )
}
