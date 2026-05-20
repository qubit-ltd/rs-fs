use qubit_fs::{
    ChecksumPolicy,
    ReadOptions,
};

#[test]
fn test_read_options_full_configuration_is_usable() {
    let options = ReadOptions {
        offset: Some(1),
        length: Some(2),
        if_match: Some("a".to_owned()),
        if_none_match: Some("b".to_owned()),
        checksum: ChecksumPolicy::Required,
    };

    assert_eq!(Some(1), options.offset);
    assert_eq!(Some(2), options.length);
    assert_eq!(Some("a"), options.if_match.as_deref());
    assert_eq!(Some("b"), options.if_none_match.as_deref());
    assert_eq!(ChecksumPolicy::Required, options.checksum);
}
