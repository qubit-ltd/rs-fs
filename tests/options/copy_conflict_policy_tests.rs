use qubit_fs::CopyConflictPolicy;

#[test]
fn test_copy_conflict_policy_default_is_fail() {
    assert_eq!(CopyConflictPolicy::Fail, CopyConflictPolicy::default());
}
