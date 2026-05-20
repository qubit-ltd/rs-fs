use qubit_fs::{
    CopyConflictPolicy,
    CopyMode,
    CopyOptions,
    MetadataPreservePolicy,
    ProgressPolicy,
    ServerSidePreference,
};

#[test]
fn test_copy_options_default_and_constructors_set_modes() {
    assert_eq!(CopyMode::Auto, CopyOptions::default().mode);
    assert_eq!(CopyMode::File, CopyOptions::file().mode);
    assert_eq!(CopyMode::Tree, CopyOptions::tree().mode);
}

#[test]
fn test_copy_options_full_configuration_is_usable() {
    let options = CopyOptions {
        mode: CopyMode::Tree,
        conflict: CopyConflictPolicy::Skip,
        preserve_metadata: MetadataPreservePolicy::ProviderNative,
        server_side: ServerSidePreference::Disable,
        follow_symlinks: true,
        create_parent: true,
        continue_on_error: true,
        filter: None,
        progress: ProgressPolicy::Detailed,
    };

    assert_eq!(CopyMode::Tree, options.mode);
    assert_eq!(CopyConflictPolicy::Skip, options.conflict);
    assert!(options.follow_symlinks);
    assert!(options.create_parent);
    assert!(options.continue_on_error);
}
