#[test]
fn test_write_all_commit_failure_retains_open_writer_for_recovery() {
    let (filesystem, _, _) = crate::handle_support::filesystem(true, Vec::new());
    let failure = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
        .expect_err("injected commit failure should propagate");
    assert_eq!(qubit_fs::FsErrorKind::Io, failure.error().kind());
    assert_eq!(
        qubit_fs::WriterState::Open,
        failure.writer().expect("writer should be retained").state(),
    );
}
