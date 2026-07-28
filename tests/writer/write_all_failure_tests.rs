#[test]
fn test_write_all_success_publishes_writer() {
    let (filesystem, _, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let outcome = filesystem
        .write_all(
            &qubit_fs::Path::parse("/target").expect("path should parse"),
            b"bytes",
            qubit_fs::WriteOptions::default(),
        )
        .expect("write should commit");
    assert_eq!(qubit_fs::AchievedAtomicity::Atomic, outcome.atomicity);
}
