// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_fs::{
    FsAuthority,
    FsUri,
};

#[test]
fn test_authority_builder_sets_optional_port_and_username() {
    let authority = FsAuthority::new("bucket")
        .unwrap()
        .with_port(443)
        .with_username("alice")
        .unwrap();

    assert_eq!("bucket", authority.host());
    assert_eq!(Some(443), authority.port());
    assert_eq!(Some("alice"), authority.username());
}

#[test]
fn test_authority_builder_rejects_invalid_host_and_username() {
    for host in [
        "",
        "bad host",
        "host/path",
        "host?query",
        "host#fragment",
        "host\nname",
        "not::ipv6",
    ] {
        assert!(FsAuthority::new(host).is_err(), "host should fail: {host}");
    }
    assert!(
        FsAuthority::new("bucket")
            .unwrap()
            .with_username("")
            .is_err()
    );
    assert!(
        FsAuthority::new("bucket")
            .unwrap()
            .with_username("user:password")
            .is_err()
    );
    assert!(
        FsAuthority::new("bucket")
            .unwrap()
            .with_username("user\nname")
            .is_err()
    );
}

#[test]
fn authority_display_encodes_username_ipv6_and_port() {
    assert_eq!("bucket", FsAuthority::new("bucket").unwrap().to_string());
    assert_eq!(
        "alice%20smith@[2001:db8::1]:443",
        FsAuthority::new("2001:db8::1")
            .unwrap()
            .with_username("alice smith")
            .unwrap()
            .with_port(443)
            .to_string(),
    );
}

#[test]
fn uri_authority_accepts_valid_ipv6_and_encoded_usernames() {
    let plain = FsUri::parse("mock://[2001:db8::1]/path").unwrap();
    let plain_authority = plain.authority().unwrap();
    assert_eq!("2001:db8::1", plain_authority.host());
    assert_eq!(None, plain_authority.port());

    let with_details =
        FsUri::parse("mock://alice%20smith@[2001:db8::1]:443/path").unwrap();
    let authority = with_details.authority().unwrap();
    assert_eq!(Some("alice smith"), authority.username());
    assert_eq!(Some(443), authority.port());
    assert_eq!(
        "mock://alice%20smith@[2001:db8::1]:443/path",
        with_details.to_string(),
    );
}

#[test]
fn uri_authority_rejects_every_malformed_host_user_and_port_form() {
    let invalid = [
        "mock://a@b@host/path",
        "mock://@host/path",
        "mock://%ZZ@host/path",
        "mock://user@/path",
        "mock://[2001:db8::1/path",
        "mock://[]/path",
        "mock://[2001:db8::1]suffix/path",
        "mock://2001:db8::1/path",
        "mock://:80/path",
        "mock://host:/path",
        "mock://host:abc/path",
        "mock://host:65536/path",
        "mock://user%3Apassword@host/path",
        "mock://bad%20host/path",
    ];

    for uri in invalid {
        assert!(FsUri::parse(uri).is_err(), "URI should be rejected: {uri}");
    }
}
