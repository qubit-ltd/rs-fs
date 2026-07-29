#[test]
fn test_directory_entry_path_can_be_compared_with_requested_root() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/outside").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions::default(),
        )
        .expect("stream should open");
    let error = stream.next_entry().expect_err("outside entry must fail");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}

/// Verifies providers cannot silently ignore the requested lexical prefix.
#[test]
fn test_directory_stream_rejects_entry_outside_requested_prefix() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/other").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                prefix: Some("nested".to_owned()),
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");
    let error = stream
        .next_entry()
        .expect_err("provider must honor the requested prefix");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}

/// Verifies a nested prefix is evaluated before direct-child filtering.
#[test]
fn test_directory_stream_accepts_nested_prefix_without_recursive_option() {
    let entry = qubit_fs::DirEntry::new(
        qubit_fs::Path::parse("/root/nested/item").expect("entry should parse"),
        qubit_fs::FileKind::File,
    );
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, vec![entry]);
    let mut stream = filesystem
        .list(
            &qubit_fs::Path::parse("/root").expect("root should parse"),
            qubit_fs::ListOptions {
                prefix: Some("nested/item".to_owned()),
                ..qubit_fs::ListOptions::default()
            },
        )
        .expect("stream should open");

    let entry = stream
        .next_entry()
        .expect("nested prefix must be accepted")
        .expect("matching entry must be returned");
    assert_eq!(
        qubit_fs::Path::parse("/root/nested/item").expect("path should parse"),
        entry.path
    );
}
