use qubit_fs::FsAuthority;

#[test]
fn test_authority_builder_sets_optional_port_and_username() {
    let authority = FsAuthority::new("bucket").with_port(443).with_username("alice");

    assert_eq!("bucket", authority.host);
    assert_eq!(Some(443), authority.port);
    assert_eq!(Some("alice"), authority.username.as_deref());
}

#[test]
fn test_empty_username_is_ignored() {
    assert_eq!(None, FsAuthority::new("bucket").with_username("").username);
}
