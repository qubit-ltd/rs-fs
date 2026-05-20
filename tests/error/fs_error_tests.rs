use qubit_fs::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
};

#[test]
fn test_fs_error_carries_context_and_source() {
    let error = FsError::with_source(
        FsErrorKind::Io,
        FsOperation::Copy,
        "copy failed",
        std::io::Error::other("source"),
    )
    .with_path(FsPath::parse("/source").expect("path should parse"))
    .with_target(FsPath::parse("/target").expect("path should parse"))
    .with_provider("mock");

    assert_eq!(FsErrorKind::Io, error.kind());
    assert!(error.to_string().contains("copy failed"));
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn test_invalid_path_sets_invalid_path_kind() {
    assert_eq!(
        FsErrorKind::InvalidPath,
        FsError::invalid_path(FsOperation::ParsePath, "bad").kind(),
    );
}
