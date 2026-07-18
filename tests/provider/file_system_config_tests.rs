// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::HashMap;

use qubit_fs::{
    CredentialRef,
    FileSystemConfig,
    FsErrorKind,
    FsUri,
};
use qubit_metadata::Metadata;
use qubit_spi::ProviderSelection;

#[test]
fn full_config_keeps_selection_options_and_credential_reference() {
    let selection =
        ProviderSelection::named("mock").expect("selection should parse");
    let options =
        Metadata::new().with("endpoint", "storage.internal".to_owned());
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_selection(selection.clone())
    .with_options(options.clone())
    .expect("non-sensitive options should be accepted")
    .with_credentials(CredentialRef::Profile("production".to_owned()));

    assert_eq!("mock", config.uri().scheme().as_str());
    assert_eq!(Some(&selection), config.selection());
    assert_eq!(&options, config.options().as_metadata());
    assert!(matches!(
        config.credentials(),
        Some(CredentialRef::Profile(profile)) if profile == "production"
    ));

    let debug = format!("{config:?}");
    assert!(!debug.contains("storage.internal"));
    assert!(!debug.contains("production"));
}

#[test]
fn config_rejects_secret_bearing_option_keys() {
    for key in ["secret_key", "database_password", "x-amz-credential"] {
        let options = Metadata::new().with(key, "plaintext".to_owned());
        let error = FileSystemConfig::new(
            FsUri::parse("mock:///file.txt").expect("URI should parse"),
        )
        .with_options(options)
        .expect_err("secret options belong behind CredentialRef");

        assert_eq!(FsErrorKind::InvalidOptions, error.kind());
    }
}

#[test]
fn config_rejects_secret_keys_nested_in_json_options() {
    let options = Metadata::new().with(
        "connection",
        serde_json::json!({
            "replicas": [
                {"region": "primary"},
                {"database_password": "plaintext"}
            ]
        }),
    );
    let error = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_options(options)
    .expect_err("nested secret keys belong behind CredentialRef");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}

#[test]
fn config_accepts_non_sensitive_nested_json_options() {
    let options = Metadata::new().with(
        "connection",
        serde_json::json!({
            "replicas": [
                {"region": "primary"},
                {"region": "secondary"}
            ]
        }),
    );
    let config = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_options(options)
    .expect("nested non-sensitive options should be accepted");

    assert!(config.options().contains_key("connection"));
}

#[test]
fn config_rejects_secret_keys_nested_in_string_maps() {
    let options = Metadata::new().with(
        "connection",
        HashMap::from([("api_token".to_owned(), "plaintext".to_owned())]),
    );
    let error = FileSystemConfig::new(
        FsUri::parse("mock:///file.txt").expect("URI should parse"),
    )
    .with_options(options)
    .expect_err("nested string-map secrets belong behind CredentialRef");

    assert_eq!(FsErrorKind::InvalidOptions, error.kind());
}
