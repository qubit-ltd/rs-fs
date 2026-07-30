// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Exercises public path and URI parsers with arbitrary UTF-8 text.
//!
//! The target asserts no-panic, canonical round-trip, and credential-redaction
//! invariants. Each input is bounded to 4096 bytes so parser allocations and
//! runtime remain useful for fuzzing.

#![no_main]

use libfuzzer_sys::fuzz_target;
use qubit_fs::{
    ConnectionUri,
    Path,
    RelativePath,
    Uri,
};

/// Bounds parser and codec allocations for direct target invocation.
const MAX_FUZZ_INPUT_LEN: usize = 4096;

/// Builds a URI-safe credential value from fuzzer-controlled bytes.
///
/// # Parameters
///
/// - `data`: Arbitrary bytes supplied by the fuzzer.
///
/// # Returns
///
/// A non-empty hexadecimal string safe to embed as a URI userinfo password or
/// query value.
fn secret_text(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut secret = String::with_capacity(data.len().min(64) * 2);
    for byte in data.iter().take(64) {
        secret.push(HEX[usize::from(byte >> 4)] as char);
        secret.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    if secret.is_empty() {
        secret.push_str("00");
    }
    secret
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FUZZ_INPUT_LEN)];

    if let Ok(text) = std::str::from_utf8(data) {
        if let Ok(path) = Path::parse(text) {
            assert_eq!(
                path,
                Path::parse(path.as_str())
                    .expect("canonical path must reparse")
            );
        }
        if let Ok(path) = RelativePath::parse(text) {
            assert_eq!(
                path,
                RelativePath::parse(path.as_str())
                    .expect("canonical relative path must reparse")
            );
        }
        if let Ok(uri) = Uri::parse(text) {
            assert_eq!(
                uri,
                Uri::parse(uri.as_str()).expect("canonical URI must reparse")
            );
        }
        let _ = ConnectionUri::parse(text);
    }

    let secret = secret_text(data);
    let uri = ConnectionUri::parse(&format!(
        "s3://user:{secret}@bucket/key?token={secret}"
    ))
    .expect("generated connection URI must parse");
    let display = uri.to_string();
    let debug = format!("{uri:?}");
    assert!(uri.has_embedded_secret());
    assert!(!display.contains(&secret));
    assert!(!debug.contains(&secret));
});
