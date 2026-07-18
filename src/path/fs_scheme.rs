// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated filesystem URI scheme.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::FsResult;

use super::uri_codec::invalid_uri;

/// A lowercase validated URI scheme used for provider selection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsScheme(Box<str>);

impl FsScheme {
    /// Parses and canonicalizes a URI scheme.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error unless the scheme starts with an ASCII
    /// letter and contains only ASCII letters, digits, `+`, `-`, or `.`.
    pub fn parse(scheme: &str) -> FsResult<Self> {
        let mut bytes = scheme.bytes();
        let Some(first) = bytes.next() else {
            return Err(invalid_uri("URI scheme must not be empty"));
        };
        if !first.is_ascii_alphabetic()
            || !bytes.all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'+' | b'-' | b'.')
            })
        {
            return Err(invalid_uri("invalid URI scheme"));
        }
        Ok(Self(scheme.to_ascii_lowercase().into()))
    }

    /// Returns the lowercase scheme text.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for FsScheme {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}
