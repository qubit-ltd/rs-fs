// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Errors produced by built-in native path codecs.

use std::error::Error;
use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

/// Error returned by a built-in [`crate::NativePathCodec`].
///
/// The stored `offset` is measured in UTF-8 bytes for text, native bytes for
/// byte representations, and WTF-8 bytes for Windows surrogate data. The
/// error deliberately does not retain the complete path text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativePathCodecError {
    /// A native or decoded byte sequence is not strict UTF-8.
    InvalidUtf8 {
        /// Byte offset of the invalid UTF-8 sequence.
        offset: usize,
    },
    /// Text contains a bare, truncated, or non-hexadecimal percent escape.
    InvalidEscape {
        /// UTF-8 byte offset of the percent escape.
        offset: usize,
    },
    /// Text has a valid decoding but is not the unique canonical spelling.
    NonCanonicalText {
        /// UTF-8 byte offset of the first non-canonical byte.
        offset: usize,
    },
    /// Windows native text contains malformed WTF-8 data.
    InvalidWtf8 {
        /// WTF-8 byte offset of the malformed sequence.
        offset: usize,
    },
    /// The target lacks a stable lossless native-string conversion.
    UnsupportedNativeEncoding,
}

impl Display for NativePathCodecError {
    /// Formats a path-safe diagnostic without including the complete path.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::InvalidUtf8 { offset } => {
                write!(
                    formatter,
                    "invalid UTF-8 at native byte offset {offset}"
                )
            }
            Self::InvalidEscape { offset } => {
                write!(
                    formatter,
                    "invalid percent escape at UTF-8 byte offset {offset}"
                )
            }
            Self::NonCanonicalText { offset } => write!(
                formatter,
                "non-canonical native path text at UTF-8 byte offset {offset}"
            ),
            Self::InvalidWtf8 { offset } => {
                write!(formatter, "invalid WTF-8 at WTF-8 byte offset {offset}")
            }
            Self::UnsupportedNativeEncoding => formatter
                .write_str("native path encoding is not losslessly supported"),
        }
    }
}

impl Error for NativePathCodecError {}
