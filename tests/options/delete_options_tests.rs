use qubit_fs::DeleteOptions;

#[test]
fn test_delete_options_full_configuration_is_usable() {
    let options = DeleteOptions {
        recursive: true,
        missing_ok: true,
        if_match: Some("v1".to_owned()),
    };

    assert!(options.recursive);
    assert!(options.missing_ok);
    assert_eq!(Some("v1"), options.if_match.as_deref());
}
