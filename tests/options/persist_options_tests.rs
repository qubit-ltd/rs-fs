use qubit_fs::{
    AtomicityRequirement,
    MetadataPreservePolicy,
    PersistOptions,
};

#[test]
fn test_persist_options_full_configuration_is_usable() {
    let options = PersistOptions {
        overwrite: true,
        atomic: AtomicityRequirement::BestEffort,
        allow_copy_delete: true,
        preserve_metadata: MetadataPreservePolicy::All,
    };

    assert!(options.overwrite);
    assert!(options.allow_copy_delete);
    assert_eq!(AtomicityRequirement::BestEffort, options.atomic);
}
