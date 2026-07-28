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

/// Verifies connection rendering preserves a userinfo authority while masking
/// its password.
#[test]
fn test_connection_uri_preserves_authority_and_redacts_userinfo_password() {
    let uri = ConnectionUri::parse("s3://user:secret@[::1]:9000/key")
        .expect("connection URI should parse");
    let rendered = uri.to_string();
    assert!(rendered.starts_with("s3://user:"));
    assert!(rendered.contains("@[::1]:9000/key"));
    assert!(!rendered.contains("secret"));
}
