// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{FsAuthority, FsScheme, FsUri, FsUriPath, FsUriQuery};

#[test]
fn test_uri_preserves_encoded_path_and_repeated_query_values() {
    let uri = FsUri::parse("mock://bucket/a%2Fb/../c%20d?tag=one&tag=two&empty=").unwrap();

    assert_eq!("/a%2Fb/../c%20d", uri.path().as_encoded());
    assert_eq!(vec!["one", "two"], uri.query().get_all("tag"));
    assert_eq!(vec![""], uri.query().get_all("empty"));
    assert_eq!(
        "mock://bucket/a%2Fb/../c%20d?tag=one&tag=two&empty=",
        uri.to_string(),
    );
}

#[test]
fn test_uri_path_decodes_validated_percent_encoded_text() {
    let path = FsUriPath::parse("/caf%C3%A9%2F100%25").expect("path should parse");

    assert_eq!("/café/100%", path.decode());
}

#[test]
fn test_uri_rejects_unsafe_or_ambiguous_syntax() {
    assert!(FsUri::parse("mock://user:password@host/path").is_err());
    assert!(FsUri::parse("mock://host/path?access_key=secret").is_err());
    assert!(FsUri::parse("mock://host/path#fragment").is_err());
    assert!(FsUri::parse("mock://host/bad%2").is_err());
    assert!(FsUri::parse("mock://host/bad%FF").is_err());
    assert!(FsUri::parse("mock://host/bad%00").is_err());
    assert!(FsUri::parse("mock://host/path\n").is_err());
    assert!(FsUri::parse("1mock:/path").is_err());
    assert!(FsUri::parse("mock:?").is_err());
}

#[test]
fn test_parse_uri_with_authority_port_user_path_and_query() {
    let uri = FsUri::parse("mock://user@example.com:8080/root/file.txt?region=test")
        .expect("URI should parse");
    let authority = uri.authority().expect("authority should exist");

    assert_eq!("mock", uri.scheme().as_str());
    assert_eq!("example.com", authority.host());
    assert_eq!(Some(8080), authority.port());
    assert_eq!(Some("user"), authority.username());
    assert_eq!("/root/file.txt", uri.path().as_encoded());
    assert_eq!(vec!["test"], uri.query().get_all("region"));
}

#[test]
fn test_parse_uri_rejects_invalid_uri_strings() {
    assert!(FsUri::parse("not a uri").is_err());
    assert!(FsUri::parse("mock:").is_err());
}

#[test]
fn test_parse_uri_supports_missing_authority_and_host_only_authority() {
    let no_authority = FsUri::parse("mock:/plain").expect("URI without authority should parse");
    assert!(no_authority.authority().is_none());
    assert!(!no_authority.has_authority_component());
    assert_eq!("/plain", no_authority.path().as_encoded());
    assert_eq!("mock:/plain", no_authority.to_string());

    let host_without_details =
        FsUri::parse("mock://bucket/root").expect("host-only URI should parse");
    let host_authority = host_without_details
        .authority()
        .expect("authority should exist");
    assert_eq!("bucket", host_authority.host());
    assert_eq!(None, host_authority.port());
    assert_eq!(None, host_authority.username());
    assert!(host_without_details.has_authority_component());

    let empty_authority = FsUri::parse("file:///tmp/data").expect("empty authority should parse");
    assert!(empty_authority.has_authority_component());
    assert!(empty_authority.authority().is_none());
    assert_eq!("file:///tmp/data", empty_authority.to_string());

    let authority_only = FsUri::parse("mock://bucket").expect("authority-only URI should parse");
    assert_eq!("/", authority_only.path().as_encoded());
    assert_eq!("mock://bucket/", authority_only.to_string());

    let empty_authority_only =
        FsUri::parse("mock://").expect("empty authority-only URI should parse");
    assert!(empty_authority_only.has_authority_component());
    assert!(empty_authority_only.authority().is_none());
    assert_eq!("mock:///", empty_authority_only.to_string());
}

#[test]
fn uri_can_be_built_from_validated_components() {
    let uri = FsUri::new(
        FsScheme::parse("MOCK").unwrap(),
        Some(FsAuthority::new("bucket").unwrap()),
        FsUriPath::parse("/a%2fb").unwrap(),
        FsUriQuery::parse("tag=one&tag=two").unwrap(),
    )
    .unwrap();

    assert_eq!("mock", uri.scheme().as_str());
    assert!(uri.has_authority_component());
    assert_eq!("/a%2Fb", uri.path().as_encoded());
    assert_eq!("mock://bucket/a%2Fb?tag=one&tag=two", uri.to_string());
}

#[test]
fn uri_component_builder_rejects_ambiguous_authority_path_combinations() {
    let scheme = FsScheme::parse("mock").unwrap();
    let query = FsUriQuery::default();

    assert!(
        FsUri::new(
            scheme.clone(),
            Some(FsAuthority::new("bucket").unwrap()),
            FsUriPath::parse("relative").unwrap(),
            query.clone(),
        )
        .is_err()
    );
    assert!(
        FsUri::new(
            scheme,
            None,
            FsUriPath::parse("//ambiguous").unwrap(),
            query,
        )
        .is_err()
    );
}

#[test]
fn uri_component_types_validate_iterate_and_display_canonical_text() {
    assert!(FsScheme::parse("").is_err());
    assert!(FsScheme::parse("1mock").is_err());
    assert!(FsScheme::parse("mock_").is_err());
    assert_eq!("mock+v1", FsScheme::parse("MOCK+V1").unwrap().to_string());

    assert!(FsUriPath::parse("").is_err());
    assert!(FsUriPath::parse("/raw space").is_err());
    assert!(FsUriPath::parse("/raw?query").is_err());
    assert!(FsUriPath::parse("/raw#fragment").is_err());
    assert!(FsUriPath::parse("/rawé").is_err());
    assert_eq!(
        "/caf%C3%A9%2Ffile",
        FsUriPath::parse("/caf%c3%a9%2ffile").unwrap().to_string(),
    );

    let empty_query = FsUriQuery::parse("").unwrap();
    assert!(empty_query.is_empty());
    assert_eq!("", empty_query.to_string());

    let query = FsUriQuery::parse("flag&x=a%20b&x=%7e&plus=+&unicode=é").unwrap();
    assert_eq!(
        vec![
            ("flag", ""),
            ("x", "a b"),
            ("x", "~"),
            ("plus", "+"),
            ("unicode", "é"),
        ],
        query.iter().collect::<Vec<_>>(),
    );
    assert_eq!(
        "flag=&x=a%20b&x=~&plus=%2B&unicode=%C3%A9",
        query.to_string(),
    );
}

#[test]
fn uri_query_rejects_empty_malformed_control_and_sensitive_pairs() {
    let invalid = [
        "=value",
        "%ZZ=value",
        "key=%ZZ",
        "key=%00",
        "password=value",
        "pass_word=value",
        "passwd=value",
        "token=value",
        "access-token=value",
        "access.key=value",
        "secret=value",
        "secret_key=value",
        "api-key=value",
        "credential=value",
        "credentials=value",
        "access%20token=value",
        "secret%2Ekey=value",
        "database_password=value",
        "x-amz-security-token=value",
        "x-amz-signature=value",
        "x-goog-credential=value",
        "sig=value",
    ];

    for query in invalid {
        assert!(
            FsUriQuery::parse(query).is_err(),
            "query should be rejected: {query}",
        );
    }
}
