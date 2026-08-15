// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::metadata::ResourceVersion;

#[test]
fn resource_version_preserves_and_displays_opaque_text() {
    let version = ResourceVersion::from("etag-42");

    assert_eq!("etag-42", version.as_str());
    assert_eq!("etag-42", version.as_ref());
    assert_eq!("etag-42", version.to_string());
}

/// Verifies owned provider version text can be converted without changing its
/// opaque representation.
#[test]
fn test_resource_version_converts_owned_text() {
    let version = ResourceVersion::from(String::from("generation-8"));

    assert_eq!("generation-8", version.as_str());
    assert_eq!("generation-8", version.to_string());
}
