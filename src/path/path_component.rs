// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! One validated logical path component.

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::error::FsOperation;
use crate::error::FsResult;

/// A non-empty logical component that cannot express hierarchy or traversal.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PathComponent(
    /// Validated component text containing no hierarchy or traversal marker.
    String,
);

impl PathComponent {
    /// Parses one logical component.
    ///
    /// Returns an invalid-path error for empty input, separators, traversal
    /// markers, or NUL. This method performs no native-path conversion.
    pub fn parse(text: &str) -> FsResult<Self> {
        if text.is_empty() || matches!(text, "." | "..") || text.contains('/') || text.contains('\0') {
            return Err(FsError::invalid_path(
                FsOperation::ParsePath,
                "path component must be a non-empty non-traversal component",
            ));
        }
        Ok(Self(text.to_owned()))
    }

    /// Returns the validated logical component text.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PathComponent {
    /// Formats the validated component without changing its lexical spelling.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}
