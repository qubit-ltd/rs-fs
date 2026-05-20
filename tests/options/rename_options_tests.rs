use qubit_fs::{
    AtomicityRequirement,
    RenameOptions,
};

#[test]
fn test_rename_options_full_configuration_and_default_are_usable() {
    let options = RenameOptions {
        overwrite: true,
        atomic: AtomicityRequirement::Required,
    };

    assert!(options.overwrite);
    assert_eq!(AtomicityRequirement::Required, options.atomic);
    assert_eq!(
        RenameOptions {
            overwrite: false,
            atomic: AtomicityRequirement::BestEffort,
        },
        RenameOptions::default(),
    );
}
