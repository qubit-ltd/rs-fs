use qubit_fs::CreateDirOptions;

#[test]
fn test_create_dir_options_full_configuration_is_usable() {
    let options = CreateDirOptions {
        recursive: true,
        exists_ok: true,
        user_metadata: qubit_metadata::Metadata::new(),
    };

    assert!(options.recursive);
    assert!(options.exists_ok);
}
