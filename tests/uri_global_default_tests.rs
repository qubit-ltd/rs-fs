// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests URI credential boundaries against application default policy changes.

use qubit_fs::{
    ConnectionUri,
    Uri,
};
use qubit_redact::RedactionPolicy;

/// Verifies URI credential boundaries cannot be disabled by an application
/// allow rule in the process-wide redaction default.
#[test]
fn test_uri_credential_boundaries_ignore_global_allow_rules() {
    let policy = RedactionPolicy::builder()
        .allow_canonical_exact("password")
        .expect("password is a valid field name")
        .allow_canonical_exact("token")
        .expect("token is a valid field name")
        .build()
        .expect("the policy is valid");
    RedactionPolicy::install_global(policy)
        .expect("this test process installs its default only once");

    assert!(Uri::parse("s3://bucket/key?token=raw-token").is_err());

    let connection = ConnectionUri::parse(
        "s3://user:raw-password@bucket/key?token=raw-token",
    )
    .expect("connection URI should parse");
    let rendered = connection.to_string();
    assert!(!rendered.contains("raw-password"));
    assert!(!rendered.contains("raw-token"));
}
