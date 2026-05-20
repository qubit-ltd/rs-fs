use qubit_fs::{
    FileSystemMetadata,
    PathSemantics,
};

#[test]
fn test_file_system_metadata_new_sets_provider_and_path_semantics() {
    let metadata = FileSystemMetadata::new("mock");

    assert_eq!("mock", metadata.provider_id);
    assert_eq!(PathSemantics::Hierarchical, metadata.path_semantics);
}
