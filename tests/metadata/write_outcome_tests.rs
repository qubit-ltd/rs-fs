use qubit_fs::WriteOutcome;

#[test]
fn test_write_outcome_new_has_no_byte_count() {
    let outcome = WriteOutcome::new();

    assert_eq!(None, outcome.bytes_written);
}
