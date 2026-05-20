use qubit_fs::{
    FileKind,
    FileMetadata,
};

#[test]
fn test_is_directory_like_matches_directory_and_prefix() {
    assert!(FileMetadata::new(FileKind::Directory).is_directory_like());
    assert!(FileMetadata::new(FileKind::Prefix).is_directory_like());
    assert!(!FileMetadata::new(FileKind::File).is_directory_like());
}
