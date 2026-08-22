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
use qubit_redact::Redactor;
use qubit_redact::Sensitivity;

/// Verifies URI parsing uses an explicit policy instead of process-global
/// redaction state.
#[test]
fn test_uri_credential_boundaries_ignore_global_allow_rules() {
    let policy = RedactionPolicy::builder()
        .fields(|fields| {
            fields.raise("tenant_payload", Sensitivity::Secret);
        })
        .expect("tenant_payload is a valid field name")
        .build()
        .expect("the policy is valid");
    let previous =
        Redactor::replace_application_default(Redactor::new(policy.clone()));

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

    let redaction = Redactor::new(policy)
        .redact_uri("s3://bucket/key?tenant_payload=raw-secret");
    assert_ne!(
        "s3://bucket/key?tenant_payload=raw-secret",
        redaction.text().as_str(),
    );
    assert!(!redaction.text().as_str().contains("raw-secret"));
    assert_eq!(
        RedactionCompletion::Complete,
        redaction.summary().completion()
    );
    let _ = Redactor::replace_application_default(previous);
}
