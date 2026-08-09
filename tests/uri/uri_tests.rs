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

/// Verifies secret-free URIs allow a visible username but reject passwords.
#[test]
fn test_uri_allows_username_only_userinfo() {
    let uri = Uri::parse("s3://user@bucket/key")
        .expect("username-only URI should remain visible and valid");
    assert_eq!(uri.authority(), Some("user@bucket"));
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
    let uri = Uri::parse("S3://bucket/a%2Fb?x=1&x=2").expect("URI should parse");
    assert_eq!(uri.scheme(), "s3");
    assert_eq!(uri.path(), "/a%2Fb");
    assert_eq!(uri.query(), Some("x=1&x=2"));
}

/// Verifies a canonical secret-free URI spelling can be parsed again without
/// changing URI identity.
#[test]
fn test_uri_canonical_text_round_trips() {
    let uri = Uri::parse("S3://bucket/a%2Fb?x=1&x=2").expect("URI should parse");
    let reparsed = Uri::parse(uri.as_str()).expect("canonical URI should reparse");
    assert_eq!(uri, reparsed);
}

/// Verifies parsed URI accessors distinguish a missing authority and query
/// while preserving the complete canonical spelling.
#[test]
fn test_uri_accessors_preserve_authority_presence_and_full_text() {
    let without_authority = Uri::parse("S3:/object").expect("URI should parse");
    assert_eq!("s3", without_authority.scheme());
    assert_eq!(None, without_authority.authority());
    assert!(!without_authority.has_authority());
    assert_eq!(None, without_authority.query());
    assert_eq!("s3:/object", without_authority.as_str());

    let empty_authority = Uri::parse("s3:///object").expect("URI should parse");
    assert!(empty_authority.has_authority());
    assert_eq!(Some(""), empty_authority.authority());
}

/// Verifies URI parser failures report missing, empty, and malformed schemes
/// before a value reaches the secret-free URI boundary.
#[test]
fn test_uri_rejects_missing_empty_and_malformed_scheme_forms() {
    assert!(Uri::parse("object-only").is_err());
    assert!(Uri::parse(":/object").is_err());
    assert!(Uri::parse("1bad:/object").is_err());
}

/// Verifies uppercase hexadecimal escapes are decoded before sensitive query
/// key classification, just like lowercase escapes.
#[test]
fn test_uri_rejects_uppercase_percent_encoded_sensitive_query_key() {
    assert!(Uri::parse("s3://bucket/key?t%6Fken=secret").is_err());
}
