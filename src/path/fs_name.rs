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

use super::native_path_text::validate_canonical_text;

/// A non-empty canonical native-path component that cannot escape its parent.
///
/// The text follows the same `%XX` invariant as [`crate::FsPath`] and
/// [`crate::NativePathCodec`]: a native percent sign is `%25`, and bytes that
/// cannot appear literally are uppercase escapes. This is canonical path text,
/// not URI encoding or a lossy display string.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsName(Box<str>);

impl FsName {
    /// Parses one safe filesystem name.
    ///
    /// # Errors
    ///
    /// Returns [`FsError`] when `name` is empty, is `.` or `..`, contains a
    /// separator, malformed or non-canonical native-path escaping, NUL, or
    /// another literal control character.
    pub fn parse(name: &str) -> FsResult<Self> {
        validate_name_text(name)?;
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

/// Validates the shared canonical native-path text invariant.
///
/// # Errors
///
/// Returns an invalid-path error when `name` has malformed or non-canonical
/// native-path escaping.
fn validate_name_text(name: &str) -> FsResult<()> {
    validate_canonical_text(name).map_err(|_| {
        FsError::invalid_path(
            FsOperation::ParsePath,
            "name text must use canonical native-path escaping",
        )
    })
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
