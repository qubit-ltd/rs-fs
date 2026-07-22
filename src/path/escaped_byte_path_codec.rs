// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Codec for arbitrary native path bytes.

use std::borrow::Cow;

use crate::{
    NativePathCodec,
    NativePathCodecError,
};

use super::native_path_text::{
    decode_canonical_text,
    encode_path_bytes,
};

/// Losslessly maps arbitrary bytes to canonical native-path text.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct EscapedBytePathCodec;

impl NativePathCodec for EscapedBytePathCodec {
    type Native = [u8];
    type Error = NativePathCodecError;

    /// Decodes canonical text into arbitrary native bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `text` is not canonical native-path text.
    fn encode<'a>(&self, text: &'a str) -> Result<Cow<'a, [u8]>, Self::Error> {
        let bytes = decode_canonical_text(text)?;
        if text.as_bytes().contains(&b'%') {
            Ok(Cow::Owned(bytes))
        } else {
            Ok(Cow::Borrowed(text.as_bytes()))
        }
    }

    /// Decodes arbitrary native bytes into canonical text.
    fn decode<'a>(
        &self,
        native: &'a [u8],
    ) -> Result<Cow<'a, str>, Self::Error> {
        let text = encode_path_bytes(native);
        if let Ok(native_text) = std::str::from_utf8(native)
            && native_text == text
        {
            Ok(Cow::Borrowed(native_text))
        } else {
            Ok(Cow::Owned(text))
        }
    }
}
