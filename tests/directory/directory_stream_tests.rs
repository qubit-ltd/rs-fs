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
