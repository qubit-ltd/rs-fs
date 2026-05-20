use qubit_fs::FileKind;

#[test]
fn test_file_kind_variants_are_comparable() {
    let kinds = [
        FileKind::File,
        FileKind::Directory,
        FileKind::Symlink,
        FileKind::Object,
        FileKind::Prefix,
        FileKind::Other("custom".to_owned()),
    ];

    assert!(kinds.contains(&FileKind::File));
    assert!(kinds.contains(&FileKind::Directory));
    assert!(kinds.contains(&FileKind::Symlink));
    assert!(kinds.contains(&FileKind::Object));
    assert!(kinds.contains(&FileKind::Prefix));
    assert!(kinds.contains(&FileKind::Other("custom".to_owned())));
}
