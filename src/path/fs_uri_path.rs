// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Raw encoded filesystem URI path.

use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::FsResult;

use super::uri_codec::canonicalize_encoded;
use super::uri_codec::percent_decode;

/// A validated URI path that retains percent-encoded path boundaries.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FsUriPath(Box<str>);

impl FsUriPath {
    /// Parses a raw encoded URI path without decoding `%2F` or dot segments.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error for empty paths, invalid percent escapes,
    /// invalid UTF-8 after decoding, or control characters.
    pub fn parse(encoded: &str) -> FsResult<Self> {
        if encoded.is_empty() {
            return Err(super::uri_codec::invalid_uri(
                "filesystem URI path must not be empty",
            ));
        }
        if !encoded.bytes().all(is_uri_path_byte) {
            return Err(super::uri_codec::invalid_uri(
                "filesystem URI path contains an unencoded character",
            ));
        }
        canonicalize_encoded(encoded).map(|value| Self(value.into()))
    }

    /// Returns the canonical raw encoded path.
    #[inline]
    #[must_use]
    pub fn as_encoded(&self) -> &str {
        &self.0
    }

    /// Decodes the canonical percent-encoded path as UTF-8 text.
    ///
    /// This operation removes URI encoding only. It does not split path
    /// components, normalize dot segments, or apply provider path semantics.
    ///
    /// # Returns
    ///
    /// The decoded path text, including any encoded separators such as `%2F`.
    ///
    /// # Panics
    ///
    /// Panics only if this type's validated percent-encoding invariant is
    /// violated internally.
    #[inline(always)]
    #[must_use]
    pub fn decode(&self) -> String {
        percent_decode(self.as_encoded()).expect("validated filesystem URI path must decode")
    }
}

/// Returns whether one byte may appear literally in an RFC 3986 path.
fn is_uri_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
                | b'/'
                | b'%'
        )
}

impl Display for FsUriPath {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_encoded())
    }
}
