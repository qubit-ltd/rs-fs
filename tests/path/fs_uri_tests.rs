use qubit_fs::FsUri;

#[test]
fn test_parse_uri_with_authority_port_user_path_and_query() {
    let uri = FsUri::parse("mock://user@example.com:8080/root/file.txt?region=test")
        .expect("URI should parse");
    let authority = uri.authority.expect("authority should exist");

    assert_eq!("mock", uri.scheme);
    assert_eq!("example.com", authority.host);
    assert_eq!(Some(8080), authority.port);
    assert_eq!(Some("user"), authority.username.as_deref());
    assert_eq!("/root/file.txt", uri.path.as_str());
    assert_eq!(Some(String::from("test")), uri.query.get("region"));
}

#[test]
fn test_parse_uri_rejects_invalid_uri_strings() {
    assert!(FsUri::parse("not a uri").is_err());
    assert!(FsUri::parse("mock:").is_err());
}

#[test]
fn test_parse_uri_supports_missing_authority_and_host_only_authority() {
    let no_authority = FsUri::parse("mock:/plain").expect("URI without authority should parse");
    assert!(no_authority.authority.is_none());
    assert_eq!("/plain", no_authority.path.as_str());

    let host_without_details =
        FsUri::parse("mock://bucket/root").expect("host-only URI should parse");
    let host_authority = host_without_details
        .authority
        .expect("authority should exist");
    assert_eq!("bucket", host_authority.host);
    assert_eq!(None, host_authority.port);
    assert_eq!(None, host_authority.username);
}
