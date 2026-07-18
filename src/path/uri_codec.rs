// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use crate::{
    FsError,
    FsOperation,
    FsResult,
};

/// Validates percent encoding and returns a canonical encoded representation.
pub(super) fn canonicalize_encoded(value: &str) -> FsResult<String> {
    let _ = percent_decode(value)?;
    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            output.push('%');
            output.push(hex_digit(hex_value(bytes[index + 1])?));
            output.push(hex_digit(hex_value(bytes[index + 2])?));
            index += 3;
        } else {
            let character = value[index..]
                .chars()
                .next()
                .expect("index remains inside the URI component");
            output.push(character);
            index += character.len_utf8();
        }
    }
    Ok(output)
}

/// Percent-decodes a URI component and validates UTF-8 and control characters.
pub(super) fn percent_decode(value: &str) -> FsResult<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err(invalid_uri("incomplete percent escape"));
                }
                let high = hex_value(bytes[index + 1])?;
                let low = hex_value(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    let decoded = String::from_utf8(decoded)
        .map_err(|_| invalid_uri("URI component is not valid UTF-8"))?;
    if decoded.chars().any(char::is_control) {
        return Err(invalid_uri(
            "URI component must not contain control characters",
        ));
    }
    Ok(decoded)
}

/// Percent-encodes a decoded query component in canonical form.
pub(super) fn percent_encode_query(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~')
        {
            output.push(char::from(*byte));
        } else {
            output.push('%');
            output.push(hex_digit(byte >> 4));
            output.push(hex_digit(byte & 0x0f));
        }
    }
    output
}

/// Creates a typed invalid-URI error.
pub(super) fn invalid_uri(message: &str) -> FsError {
    FsError::new(
        crate::FsErrorKind::InvalidUri,
        FsOperation::ParseUri,
        message,
    )
}

/// Converts one ASCII hexadecimal digit into its numeric value.
fn hex_value(value: u8) -> FsResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(invalid_uri("invalid percent escape")),
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
