use qubit_fs::{
    FileMetadata,
    FileType,
};

#[test]
fn test_is_directory_like_matches_directory_and_prefix() {
    assert!(FileMetadata::new(FileType::Directory).is_directory_like());
    assert!(FileMetadata::new(FileType::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileType::File).is_directory_like());
}
