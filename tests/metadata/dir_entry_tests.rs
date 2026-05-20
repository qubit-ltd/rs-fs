use qubit_fs::{
    DirEntry,
    FileType,
    FsPath,
};

#[test]
fn test_dir_entry_new_derives_file_name() {
    let entry = DirEntry::new(
        FsPath::parse("/dir/file.txt").expect("path should parse"),
        FileType::File,
    );

    assert_eq!("file.txt", entry.name);
    assert_eq!(FileType::File, entry.file_type);
}
