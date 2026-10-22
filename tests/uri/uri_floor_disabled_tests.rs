// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests URI credential boundaries with an explicitly disabled redaction floor.

use qubit_fs::path::ConnectionUri;
use qubit_fs::path::Uri;
use qubit_redact::RedactionCompletion;
use qubit_redact::RedactionPolicy;
use qubit_redact::Redactor;

/// Verifies a weakened policy cannot disable the standard URI safety floor.
#[test]
fn test_resource_uri_keeps_standard_floor_under_weakened_policy() {
    let weakened = RedactionPolicy::builder()
        .fields(|fields| {
            fields.disable_floor();
        })
        .expect("valid fields")
        .build()
        .expect("valid policy");
    for policy in [weakened, RedactionPolicy::disabled()] {
        for text in [
            "s3://bucket/key?token=raw-token",
            "s3://bucket/key?t%6fken=raw-token",
            "s3://bucket/key?token=",
            "s3://user:raw-password@bucket/key",
        ] {
            assert!(Uri::parse_with_policy(text, &policy).is_err());
            let connection =
                ConnectionUri::parse_with_policy(text, &policy).expect("connection URI can contain credentials");
            assert!(connection.has_embedded_secret());
            assert!(connection.try_to_uri().is_err());
            assert!(!connection.to_string().contains("raw-token"));
            assert!(!format!("{connection:?}").contains("raw-password"));
        }
        assert!(Uri::parse_with_policy("s3://user@bucket/key?region=cn", &policy).is_ok());
    }

    let redaction = Redactor::new(RedactionPolicy::disabled()).redact_uri("s3://bucket/key?token=raw-token");
    assert_eq!("s3://bucket/key?token=raw-token", redaction.text().as_str());
    assert_eq!(RedactionCompletion::Complete, redaction.summary().completion());
}
