// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared canonical native-path text conversion helpers.

use super::native_path_codec_error::NativePathCodecError;

/// Decodes and validates canonical native-path text into its original bytes.
///
/// # Errors
///
/// Returns [`NativePathCodecError::InvalidEscape`] for malformed escapes and
/// [`NativePathCodecError::NonCanonicalText`] when the decoded bytes have a
/// different canonical spelling.
pub(super) fn decode_canonical_text(
    text: &str,
) -> Result<Vec<u8>, NativePathCodecError> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return Err(NativePathCodecError::InvalidEscape {
                    offset: index,
                });
            }
            let high = hex_value(bytes[index + 1])
                .ok_or(NativePathCodecError::InvalidEscape { offset: index })?;
            let low = hex_value(bytes[index + 2])
                .ok_or(NativePathCodecError::InvalidEscape { offset: index })?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            let character = text[index..]
                .chars()
                .next()
                .expect("index remains inside the UTF-8 text");
            let mut buffer = [0_u8; 4];
            decoded.extend_from_slice(
                character.encode_utf8(&mut buffer).as_bytes(),
            );
            index += character.len_utf8();
        }
    }
    let canonical = encode_path_bytes(&decoded);
    if canonical != text {
        return Err(NativePathCodecError::NonCanonicalText {
            offset: first_difference(text.as_bytes(), canonical.as_bytes()),
        });
    }
    Ok(decoded)
}

/// Validates that `text` is canonical native-path text.
///
/// # Errors
///
/// Returns the same error as [`decode_canonical_text`] without exposing the
/// decoded bytes to the caller.
pub(super) fn validate_canonical_text(
    text: &str,
) -> Result<(), NativePathCodecError> {
    let _ = decode_canonical_text(text)?;
    Ok(())
}

/// Encodes arbitrary native bytes as canonical UTF-8 path text.
#[must_use]
pub(super) fn encode_path_bytes(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match std::str::from_utf8(&bytes[index..]) {
            Ok(valid) => {
                append_valid_utf8(&mut output, valid);
                break;
            }
            Err(error) => {
                let valid_end = index + error.valid_up_to();
                let valid = std::str::from_utf8(&bytes[index..valid_end])
                    .expect("valid UTF-8 prefix reported by Utf8Error");
                append_valid_utf8(&mut output, valid);
                index = valid_end;
                if index < bytes.len() {
                    append_escape(&mut output, bytes[index]);
                    index += 1;
                }
            }
        }
    }
    output
}

/// Appends a valid UTF-8 slice using the canonical escaping rules.
fn append_valid_utf8(output: &mut String, text: &str) {
    for character in text.chars() {
        if character == '%' || character.is_control() {
            let mut buffer = [0_u8; 4];
            for byte in character.encode_utf8(&mut buffer).as_bytes() {
                append_escape(output, *byte);
            }
        } else {
            output.push(character);
        }
    }
}

/// Appends one byte as an uppercase percent escape.
fn append_escape(output: &mut String, byte: u8) {
    output.push('%');
    output.push(hex_digit(byte >> 4));
    output.push(hex_digit(byte & 0x0f));
}

/// Converts one ASCII hexadecimal digit into its numeric value.
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Converts a nibble into an uppercase hexadecimal digit.
fn hex_digit(value: u8) -> char {
    debug_assert!(value < 16, "hexadecimal nibble must fit four bits");
    match value {
        0..=9 => char::from(b'0' + value),
        _ => char::from(b'A' + value - 10),
    }
}

/// Returns the first byte offset at which two byte sequences differ.
fn first_difference(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .unwrap_or(left.len().min(right.len()))
}
