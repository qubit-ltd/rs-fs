use qubit_fs::{
    CredentialRef,
    FileSystemConfig,
    FsUri,
};

#[test]
fn test_file_system_config_stores_uri_options_and_credentials() {
    let config = FileSystemConfig {
        uri: FsUri::parse("mock:///file.txt").expect("URI should parse"),
        options: qubit_metadata::Metadata::new(),
        credentials: Some(CredentialRef::DefaultChain),
    };

    assert_eq!("mock", config.uri.scheme);
    assert!(matches!(
        config.credentials,
        Some(CredentialRef::DefaultChain)
    ));
}
