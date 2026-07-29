//! Tests URI credential boundaries against application default policy changes.

use qubit_fs::{ConnectionUri, Uri};
use qubit_redact::RedactionPolicy;

/// Verifies URI credential boundaries cannot be disabled by an application
/// allow rule in the process-wide redaction default.
#[test]
fn test_uri_credential_boundaries_ignore_global_allow_rules() {
    let policy = RedactionPolicy::builder()
        .allow_exact("password")
        .allow_exact("token")
        .build()
        .expect("the policy is valid");
    RedactionPolicy::set_global_default(policy)
        .expect("this test process installs its default only once");

    assert!(Uri::parse("s3://bucket/key?token=raw-token").is_err());

    let connection = ConnectionUri::parse("s3://user:raw-password@bucket/key?token=raw-token")
        .expect("connection URI should parse");
    let rendered = connection.to_string();
    assert!(!rendered.contains("raw-password"));
    assert!(!rendered.contains("raw-token"));
}
