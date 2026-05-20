use qubit_fs::{
    DirEntry,
    FileKind,
    FsPath,
};

#[test]
fn test_dir_entry_new_derives_file_name() {
    let entry = DirEntry::new(
        FsPath::parse("/dir/file.txt").expect("path should parse"),
        FileKind::File,
    );

    assert_eq!("file.txt", entry.name);
    assert_eq!(FileKind::File, entry.kind);
}
