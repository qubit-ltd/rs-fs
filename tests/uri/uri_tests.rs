// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for secret-free resource URIs.

use qubit_fs::Uri;

/// Verifies URI parsing preserves RFC 3986 authority lexical distinctions.
#[test]
fn test_uri_preserves_rfc3986_lexical_distinctions() {
    let one = Uri::parse("file:/tmp/a").expect("URI should parse");
    let three = Uri::parse("file:///tmp/a").expect("URI should parse");
    assert_ne!(one, three);
    assert_eq!(one.to_string(), "file:/tmp/a");
    assert_eq!(three.to_string(), "file:///tmp/a");
}

/// Verifies ordinary URIs reject all credential-bearing locations.
#[test]
fn test_uri_rejects_password_sensitive_query_and_fragment() {
    assert!(Uri::parse("s3://user:secret@bucket/key").is_err());
    assert!(Uri::parse("s3://bucket/key?token=secret").is_err());
    assert!(Uri::parse("s3://bucket/key#fragment").is_err());
}

/// Verifies secret-free URIs reject all userinfo, including username-only
/// forms.
#[test]
fn test_uri_rejects_any_userinfo() {
    assert!(Uri::parse("s3://user@bucket/key").is_err());
}

/// Verifies percent-encoded sensitive query keys are rejected before a
/// secret-free URI crosses the public boundary.
#[test]
fn test_uri_rejects_percent_encoded_sensitive_query_key() {
    assert!(Uri::parse("s3://bucket/key?t%6fken=secret").is_err());
}

/// Verifies lexical escaped separators, normalized schemes, and query order
/// survive parsing.
#[test]
fn test_uri_preserves_raw_path_and_ordered_duplicate_query() {
    let uri =
        Uri::parse("S3://bucket/a%2Fb?x=1&x=2").expect("URI should parse");
    assert_eq!(uri.scheme(), "s3");
    assert_eq!(uri.path(), "/a%2Fb");
    assert_eq!(uri.query(), Some("x=1&x=2"));
}
