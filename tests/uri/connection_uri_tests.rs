// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests for redacted connection URIs.

use qubit_fs::ConnectionUri;

/// Verifies connection URI formatting redacts all sensitive duplicate query
/// values.
#[test]
fn test_connection_uri_redacts_password_and_duplicate_sensitive_query() {
    let uri = ConnectionUri::parse(
        "s3://user:secret@bucket/key?token=one&x=1&token=two",
    )
    .expect("connection URI should parse");
    let display = uri.to_string();
    let debug = format!("{uri:?}");
    for rendered in [display, debug] {
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("one"));
        assert!(!rendered.contains("two"));
        assert!(rendered.contains("x=1"));
    }
}

/// Verifies connection URIs keep fragments outside their credential boundary.
#[test]
fn test_connection_uri_rejects_fragment() {
    assert!(ConnectionUri::parse("s3://bucket/key#fragment").is_err());
}

/// Verifies connection rendering masks all userinfo while retaining its host.
#[test]
fn test_connection_uri_preserves_authority_and_redacts_userinfo_password() {
    let uri = ConnectionUri::parse("s3://user:secret@[::1]:9000/key")
        .expect("connection URI should parse");
    let rendered = uri.to_string();
    assert!(!rendered.contains("user"));
    assert!(rendered.contains("@[::1]:9000/key"));
    assert!(!rendered.contains("secret"));
}

/// Verifies a username-only authority is still treated as credential material.
#[test]
fn test_connection_uri_redacts_username_only_authority() {
    let uri = ConnectionUri::parse("s3://access-key@bucket/key")
        .expect("connection URI should parse");
    let rendered = uri.to_string();
    assert!(!rendered.contains("access-key"));
    assert!(rendered.contains("@bucket/key"));
}

/// Verifies escaped secret keys cannot bypass connection URI redaction.
#[test]
fn test_connection_uri_redacts_percent_encoded_sensitive_query_key() {
    let uri = ConnectionUri::parse("s3://bucket/key?t%6fken=raw-secret")
        .expect("connection URI should parse");
    let rendered = uri.to_string();
    assert!(rendered.contains("t%6fken="));
    assert!(!rendered.contains("raw-secret"));
}

/// Verifies controlled inspection receives the original URI while ordinary
/// formatting remains redacted and preserves a URI without authority.
#[test]
fn test_connection_uri_exposes_unredacted_text_only_to_callback() {
    let uri = ConnectionUri::parse("s3:/key?token=raw-secret")
        .expect("connection URI should parse");

    assert_eq!(
        "s3:/key?token=raw-secret",
        uri.expose_unredacted(str::to_owned)
    );
    assert!(!uri.to_string().contains("raw-secret"));
}
