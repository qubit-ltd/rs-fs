// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Codec for the operating system's native path string type.

use std::borrow::Cow;
use std::ffi::{
    OsStr,
    OsString,
};

#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;

use crate::{
    NativePathCodec,
    NativePathCodecError,
};

use super::native_path_text::{
    decode_canonical_text,
    encode_path_bytes,
};

/// Losslessly maps [`OsStr`] values to canonical native-path text.
///
/// On Unix this applies canonical byte escaping directly. On Windows it
/// explicitly preserves unpaired UTF-16 surrogates through WTF-8 before
/// applying the same canonical byte escaping. Other targets accept only
/// values exposed as strict Unicode by the stable standard library.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct OsStrPathCodec;

impl NativePathCodec for OsStrPathCodec {
    type Native = OsStr;
    type Error = NativePathCodecError;

    /// Encodes canonical text into an operating-system native string.
    ///
    /// # Errors
    ///
    /// Returns an error when `text` is non-canonical, cannot be represented by
    /// the target native encoding, or contains invalid WTF-8 on Windows.
    fn encode<'a>(&self, text: &'a str) -> Result<Cow<'a, OsStr>, Self::Error> {
        encode_os_str(text)
    }

    /// Decodes an operating-system native string into canonical text.
    ///
    /// # Errors
    ///
    /// Returns [`NativePathCodecError::UnsupportedNativeEncoding`] on targets
    /// without a stable lossless conversion for a non-Unicode native value.
    fn decode<'a>(
        &self,
        native: &'a OsStr,
    ) -> Result<Cow<'a, str>, Self::Error> {
        decode_os_str(native)
    }
}

/// Encodes canonical text into an operating-system native string on Unix.
#[cfg(unix)]
fn encode_os_str<'a>(
    text: &'a str,
) -> Result<Cow<'a, OsStr>, NativePathCodecError> {
    use std::os::unix::ffi::OsStringExt;

    let bytes = decode_canonical_text(text)?;
    if text.as_bytes().contains(&b'%') {
        Ok(Cow::Owned(OsString::from_vec(bytes)))
    } else {
        Ok(Cow::Borrowed(OsStr::new(text)))
    }
}

/// Decodes an operating-system native string into canonical text on Unix.
#[cfg(unix)]
fn decode_os_str<'a>(
    native: &'a OsStr,
) -> Result<Cow<'a, str>, NativePathCodecError> {
    use std::os::unix::ffi::OsStrExt;

    let bytes = native.as_bytes();
    let text = encode_path_bytes(bytes);
    if let Ok(native_text) = std::str::from_utf8(bytes)
        && native_text == text
    {
        Ok(Cow::Borrowed(native_text))
    } else {
        Ok(Cow::Owned(text))
    }
}

/// Encodes canonical text into an operating-system native string on Windows.
#[cfg(windows)]
fn encode_os_str<'a>(
    text: &'a str,
) -> Result<Cow<'a, OsStr>, NativePathCodecError> {
    let bytes = decode_canonical_text(text)?;
    if text.as_bytes().contains(&b'%') {
        let wide = wtf8_to_wide(&bytes)?;
        Ok(Cow::Owned(OsString::from_wide(&wide)))
    } else {
        Ok(Cow::Borrowed(OsStr::new(text)))
    }
}

/// Decodes an operating-system native string into canonical text on Windows.
#[cfg(windows)]
fn decode_os_str<'a>(
    native: &'a OsStr,
) -> Result<Cow<'a, str>, NativePathCodecError> {
    use std::os::windows::ffi::OsStrExt;

    let bytes = wide_to_wtf8(&native.encode_wide().collect::<Vec<_>>());
    let text = encode_path_bytes(&bytes);
    if let Some(native_text) = native.to_str()
        && native_text == text
    {
        Ok(Cow::Borrowed(native_text))
    } else {
        Ok(Cow::Owned(text))
    }
}

/// Encodes canonical text on targets with only a strict-Unicode fallback.
#[cfg(not(any(unix, windows)))]
fn encode_os_str<'a>(
    text: &'a str,
) -> Result<Cow<'a, OsStr>, NativePathCodecError> {
    let bytes = decode_canonical_text(text)?;
    let decoded = std::str::from_utf8(&bytes).map_err(|error| {
        NativePathCodecError::InvalidUtf8 {
            offset: error.valid_up_to(),
        }
    })?;
    if text.as_bytes().contains(&b'%') {
        Ok(Cow::Owned(OsString::from(decoded)))
    } else {
        Ok(Cow::Borrowed(OsStr::new(text)))
    }
}

/// Decodes native text on targets with only a strict-Unicode fallback.
#[cfg(not(any(unix, windows)))]
fn decode_os_str<'a>(
    native: &'a OsStr,
) -> Result<Cow<'a, str>, NativePathCodecError> {
    let native_text = native
        .to_str()
        .ok_or(NativePathCodecError::UnsupportedNativeEncoding)?;
    let text = encode_path_bytes(native_text.as_bytes());
    if native_text == text {
        Ok(Cow::Borrowed(native_text))
    } else {
        Ok(Cow::Owned(text))
    }
}

/// Converts UTF-16 code units into WTF-8 bytes without lossy replacement.
#[cfg(windows)]
fn wide_to_wtf8(wide: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(wide.len());
    let mut index = 0;
    while index < wide.len() {
        let unit = wide[index];
        if is_high_surrogate(unit)
            && let Some(&low) = wide.get(index + 1)
            && is_low_surrogate(low)
        {
            let scalar = 0x1_0000
                + ((u32::from(unit) - 0xd800) << 10)
                + (u32::from(low) - 0xdc00);
            append_scalar_utf8(&mut bytes, scalar);
            index += 2;
        } else {
            append_scalar_utf8(&mut bytes, u32::from(unit));
            index += 1;
        }
    }
    bytes
}

/// Converts well-formed WTF-8 bytes into UTF-16 code units.
///
/// # Errors
///
/// Returns [`NativePathCodecError::InvalidWtf8`] at the first malformed WTF-8
/// byte offset.
#[cfg(windows)]
fn wtf8_to_wide(bytes: &[u8]) -> Result<Vec<u16>, NativePathCodecError> {
    let mut wide = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let (scalar, width) = decode_wtf8_scalar(bytes, index)?;
        if (0xd800..=0xdfff).contains(&scalar) {
            wide.push(scalar as u16);
        } else if scalar <= 0xffff {
            wide.push(scalar as u16);
        } else {
            let value = scalar - 0x1_0000;
            wide.push(0xd800 + ((value >> 10) as u16));
            wide.push(0xdc00 + ((value & 0x03ff) as u16));
        }
        index += width;
    }
    Ok(wide)
}

/// Decodes one well-formed WTF-8 scalar from `bytes` at `index`.
///
/// # Errors
///
/// Returns [`NativePathCodecError::InvalidWtf8`] at `index` when the sequence
/// is overlong, truncated, has bad continuation bytes, or exceeds Unicode.
#[cfg(windows)]
fn decode_wtf8_scalar(
    bytes: &[u8],
    index: usize,
) -> Result<(u32, usize), NativePathCodecError> {
    let first = bytes[index];
    if first <= 0x7f {
        return Ok((u32::from(first), 1));
    }
    let (width, minimum, mask) = match first {
        0xc2..=0xdf => (2, 0x80, 0x1f),
        0xe0..=0xef => (3, 0x800, 0x0f),
        0xf0..=0xf4 => (4, 0x1_0000, 0x07),
        _ => return Err(NativePathCodecError::InvalidWtf8 { offset: index }),
    };
    if index + width > bytes.len() {
        return Err(NativePathCodecError::InvalidWtf8 { offset: index });
    }
    let mut scalar = u32::from(first & mask);
    for byte in &bytes[index + 1..index + width] {
        if !(0x80..=0xbf).contains(byte) {
            return Err(NativePathCodecError::InvalidWtf8 { offset: index });
        }
        scalar = (scalar << 6) | u32::from(byte & 0x3f);
    }
    if scalar < minimum || scalar > 0x10ffff {
        return Err(NativePathCodecError::InvalidWtf8 { offset: index });
    }
    Ok((scalar, width))
}

/// Appends a Unicode scalar or surrogate as its WTF-8 bytes.
#[cfg(windows)]
fn append_scalar_utf8(bytes: &mut Vec<u8>, scalar: u32) {
    if let Some(character) = char::from_u32(scalar) {
        let mut buffer = [0_u8; 4];
        bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
    } else {
        bytes.push(0xe0 | ((scalar >> 12) as u8));
        bytes.push(0x80 | (((scalar >> 6) & 0x3f) as u8));
        bytes.push(0x80 | ((scalar & 0x3f) as u8));
    }
}

/// Returns whether one UTF-16 unit is a high surrogate.
#[cfg(windows)]
fn is_high_surrogate(unit: u16) -> bool {
    (0xd800..=0xdbff).contains(&unit)
}

/// Returns whether one UTF-16 unit is a low surrogate.
#[cfg(windows)]
fn is_low_surrogate(unit: u16) -> bool {
    (0xdc00..=0xdfff).contains(&unit)
}
