use qubit_fs::WriteMode;

#[test]
fn test_write_mode_default_is_create_or_truncate() {
    assert_eq!(WriteMode::CreateOrTruncate, WriteMode::default());
}
