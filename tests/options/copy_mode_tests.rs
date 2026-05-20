use qubit_fs::CopyMode;

#[test]
fn test_copy_mode_default_is_auto() {
    assert_eq!(CopyMode::Auto, CopyMode::default());
}
