use qubit_fs::ChecksumPolicy;

#[test]
fn test_checksum_policy_default_is_none() {
    assert_eq!(ChecksumPolicy::None, ChecksumPolicy::default());
}
