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

use std::str;

use libfuzzer_sys::fuzz_target;
use qubit_fs::Path;
use qubit_fs::path::ConnectionUri;
use qubit_fs::path::PathComponent;
use qubit_fs::path::PathSemantics;
use qubit_fs::path::RelativePath;
use qubit_fs::path::Uri;

/// Bounds parser and codec allocations for direct target invocation.
const MAX_FUZZ_INPUT_LEN: usize = 4096;

/// Checks semantic round trips and composition without normalizing literal
/// keys.
///
/// # Parameters
/// - `text`: Bounded UTF-8 fuzz input, optionally separating base and suffix by
///   newline.
fn check_path_properties(text: &str) {
    let (base, suffix) = text.split_once('\n').unwrap_or((text, text));
    for semantics in [
        PathSemantics::Hierarchical,
        PathSemantics::ObjectKey,
        PathSemantics::ProviderSpecific,
    ] {
        let Ok(path) = Path::parse_with_semantics(base, semantics) else {
            continue;
        };
        assert_eq!(path, Path::parse_with_semantics(path.as_str(), semantics).unwrap());
        if semantics != PathSemantics::Hierarchical {
            assert_eq!(path.as_str(), base);
        }
        let expected = |suffix: &str| {
            if semantics == PathSemantics::Hierarchical && path.as_str() == "/" {
                format!("/{suffix}")
            } else {
                format!("{}/{suffix}", path.as_str())
            }
        };
        if let Ok(component) = PathComponent::parse(suffix) {
            let result = path.child(&component);
            assert_eq!(result.semantics(), semantics);
            assert_eq!(result.is_absolute(), path.is_absolute());
            assert_eq!(result.as_str(), expected(component.as_str()));
            assert_eq!(result, Path::parse_with_semantics(result.as_str(), semantics).unwrap());
        }
        if let Ok(relative) = RelativePath::parse(suffix) {
            let result = path.join(&relative);
            assert_eq!(result.semantics(), semantics);
            assert_eq!(result.is_absolute(), path.is_absolute());
            assert_eq!(result.as_str(), expected(relative.as_str()));
            assert_eq!(result, Path::parse_with_semantics(result.as_str(), semantics).unwrap());
        }
    }
    for absolute in [false, true] {
        if let Ok(path) = Path::from_components(absolute, text.split('/')) {
            let spelling = if absolute { format!("/{text}") } else { text.to_owned() };
            assert_eq!(path, Path::parse(&spelling).unwrap());
        }
    }
}

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
#[must_use]
fn secret_text(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut secret = String::from("fuzz-secret-");
    secret.reserve(data.len().min(64) * 2);
    for byte in data.iter().take(64) {
        secret.push(HEX[usize::from(byte >> 4)] as char);
        secret.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    secret
}

fuzz_target!(|data: &[u8]| {
    let data = &data[..data.len().min(MAX_FUZZ_INPUT_LEN)];

    if let Ok(text) = str::from_utf8(data) {
        check_path_properties(text);
        if let Ok(path) = Path::parse(text) {
            assert_eq!(path, Path::parse(path.as_str()).expect("canonical path must reparse"));
        }
        if let Ok(path) = RelativePath::parse(text) {
            assert_eq!(
                path,
                RelativePath::parse(path.as_str()).expect("canonical relative path must reparse")
            );
        }
        if let Ok(uri) = Uri::parse(text) {
            assert_eq!(uri, Uri::parse(uri.as_str()).expect("canonical URI must reparse"));
        }
        let _ = ConnectionUri::parse(text);
    }

    let secret = secret_text(data);
    let uri = ConnectionUri::parse(&format!("s3://user:{secret}@bucket/key?token={secret}"))
        .expect("generated connection URI must parse");
    let display = uri.to_string();
    let debug = format!("{uri:?}");
    assert!(uri.has_embedded_secret());
    assert!(!display.contains(&secret));
    assert!(!debug.contains(&secret));
});
