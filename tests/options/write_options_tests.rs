use qubit_fs::{
    Checksum,
    ChecksumAlgorithm,
    WriteMode,
    WriteOptions,
};

#[test]
fn test_write_options_full_configuration_is_usable() {
    let checksum = Checksum::new(ChecksumAlgorithm::Sha256, "abc");
    let options = WriteOptions {
        create_parent: true,
        mode: WriteMode::ConditionalReplace { etag: "v1".to_owned() },
        content_type: Some("text/plain".to_owned()),
        user_metadata: qubit_metadata::Metadata::new(),
        checksum: Some(checksum),
    };

    assert!(options.create_parent);
    assert_eq!(Some("text/plain"), options.content_type.as_deref());
    assert!(options.checksum.is_some());
}
