// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated single filesystem path component.

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

/// A non-empty single path component that cannot escape its parent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsName(Box<str>);

impl FsName {
    /// Parses one safe filesystem name.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `name` is empty, is `.` or `..`, contains a
    /// separator, NUL, or another control character.
    pub fn parse(name: &str) -> FsResult<Self> {
        if name.is_empty() {
            return Err(invalid_name("filesystem name must not be empty"));
        }
        if matches!(name, "." | "..") {
            return Err(invalid_name("filesystem name must not be . or .."));
        }
        if name.contains('/') {
            return Err(invalid_name("filesystem name must be one component"));
        }
        if name.chars().any(char::is_control) {
            return Err(invalid_name(
                "filesystem name must not contain control characters",
            ));
        }
        Ok(Self(name.into()))
    }

    /// Returns the validated name text.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for FsName {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

/// Creates an invalid-path error for a rejected name.
fn invalid_name(message: &str) -> FsError {
    FsError::invalid_path(FsOperation::ParsePath, message)
}
