use qubit_fs::ProgressPolicy;

#[test]
fn test_progress_policy_default_is_count_only() {
    assert_eq!(ProgressPolicy::CountOnly, ProgressPolicy::default());
}
