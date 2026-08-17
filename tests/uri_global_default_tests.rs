// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests URI credential boundaries against application default policy changes.

use qubit_fs::path::ConnectionUri;
use qubit_fs::path::Uri;
use qubit_redact::RedactionCompletion;
use qubit_redact::RedactionPolicy;
use qubit_redact::Sensitivity;
use qubit_redact::formats::uri::UriRedactionStatus;
use qubit_redact::formats::uri::UriRedactor;

/// Verifies URI parsing uses an explicit policy instead of process-global
/// redaction state.
#[test]
fn test_uri_credential_boundaries_ignore_global_allow_rules() {
    let mut builder = RedactionPolicy::builder();
    builder
        .legacy_fields()
        .raise("tenant_payload", Sensitivity::Secret)
        .expect("tenant_payload is a valid field name");
    let policy = builder.build().expect("the policy is valid");
    RedactionPolicy::install_global(policy.clone())
        .expect("this test process installs its default only once");

    assert!(Uri::parse("s3://bucket/key?tenant_payload=raw-secret").is_ok());
    assert!(
        Uri::parse_with_policy(
            "s3://bucket/key?tenant_payload=raw-secret",
            &policy
        )
        .is_err()
    );

    let connection = ConnectionUri::parse_with_policy(
        "s3://user:raw-password@bucket/key?tenant_payload=raw-secret",
        &policy,
    )
    .expect("connection URI should parse");
    let rendered = connection.to_string();
    assert!(!rendered.contains("raw-password"));
    assert!(!rendered.contains("raw-secret"));

    let redaction = UriRedactor::new(policy)
        .redact_uri_str("s3://bucket/key?tenant_payload=raw-secret");
    assert_eq!(UriRedactionStatus::Redacted, redaction.status());
    assert_eq!(RedactionCompletion::Complete, redaction.completion());
}
