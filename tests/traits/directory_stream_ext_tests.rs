use qubit_fs::{
    DirEntry,
    DirectoryStream,
    DirectoryStreamExt,
    FileKind,
    FsPath,
};

use crate::common::{
    FailingDirectoryStream,
    MockDirectoryStream,
    PartiallyFailingDirectoryStream,
};

#[test]
fn test_collect_entries_collects_all_entries() {
    let entries = (Box::new(MockDirectoryStream {
        entries: vec![DirEntry::new(
            FsPath::parse("/a.txt").expect("path should parse"),
            FileKind::File,
        )],
    }) as Box<dyn DirectoryStream>)
        .collect_entries()
        .expect("stream should collect");

    assert_eq!(1, entries.len());
}

#[test]
fn test_collect_entries_returns_empty_vec_for_empty_stream() {
    let entries = (Box::new(MockDirectoryStream { entries: Vec::new() }) as Box<dyn DirectoryStream>)
        .collect_entries()
        .expect("empty stream should collect");

    assert!(entries.is_empty());
}

#[test]
fn test_collect_entries_returns_errors_from_stream() {
    assert!(
        (Box::new(FailingDirectoryStream) as Box<dyn DirectoryStream>)
            .collect_entries()
            .is_err(),
    );
    assert!(
        (Box::new(PartiallyFailingDirectoryStream {
            entry: Some(DirEntry::new(
                FsPath::parse("/partial.txt").expect("path should parse"),
                FileKind::File,
            )),
        }) as Box<dyn DirectoryStream>)
            .collect_entries()
            .is_err(),
    );
}
