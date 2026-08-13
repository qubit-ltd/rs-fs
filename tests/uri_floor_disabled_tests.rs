// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Tests URI credential boundaries with an explicitly disabled redaction floor.

use qubit_fs::Uri;
use qubit_redact::RedactionPolicy;

/// Verifies an explicitly supplied policy controls URI query-key
/// classification without a hidden standard-policy fallback.
#[test]
fn test_uri_query_policy_respects_explicitly_disabled_floor() {
    let policy = RedactionPolicy::builder()
        .disable_floor()
        .build()
        .expect("the policy without a floor is valid");
    Uri::parse_with_policy("s3://bucket/key?token=raw-token", &policy)
        .expect("an explicitly disabled floor permits an otherwise unknown query key");
}
