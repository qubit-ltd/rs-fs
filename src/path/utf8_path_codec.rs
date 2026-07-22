// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Codec for native path strings constrained to strict UTF-8 bytes.

use std::borrow::Cow;

use crate::{
    NativePathCodec,
    NativePathCodecError,
};

use super::native_path_text::{
    decode_canonical_text,
    encode_path_bytes,
};

/// Maps strict UTF-8 native bytes to canonical native-path text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct Utf8PathCodec;

impl NativePathCodec for Utf8PathCodec {
    type Native = [u8];
    type Error = NativePathCodecError;

    /// Decodes canonical text into strict UTF-8 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `text` is non-canonical or decodes to non-UTF-8
    /// native bytes.
    fn encode<'a>(&self, text: &'a str) -> Result<Cow<'a, [u8]>, Self::Error> {
        let bytes = decode_canonical_text(text)?;
        std::str::from_utf8(&bytes).map_err(|error| {
            NativePathCodecError::InvalidUtf8 {
                offset: error.valid_up_to(),
            }
        })?;
        if text.as_bytes().contains(&b'%') {
            Ok(Cow::Owned(bytes))
        } else {
            Ok(Cow::Borrowed(text.as_bytes()))
        }
    }

    /// Decodes strict UTF-8 native bytes into canonical text.
    ///
    /// # Errors
    ///
    /// Returns [`NativePathCodecError::InvalidUtf8`] when `native` is not
    /// strict UTF-8.
    fn decode<'a>(
        &self,
        native: &'a [u8],
    ) -> Result<Cow<'a, str>, Self::Error> {
        let native_text = std::str::from_utf8(native).map_err(|error| {
            NativePathCodecError::InvalidUtf8 {
                offset: error.valid_up_to(),
            }
        })?;
        let text = encode_path_bytes(native);
        if native_text == text {
            Ok(Cow::Borrowed(native_text))
        } else {
            Ok(Cow::Owned(text))
        }
    }
}
