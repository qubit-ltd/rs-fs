use qubit_fs::MetadataPreservePolicy;

#[test]
fn test_metadata_preserve_policy_default_is_portable() {
    assert_eq!(
        MetadataPreservePolicy::Portable,
        MetadataPreservePolicy::default(),
    );
}
