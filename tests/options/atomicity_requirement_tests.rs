use qubit_fs::AtomicityRequirement;

#[test]
fn test_atomicity_requirement_default_is_best_effort() {
    assert_eq!(
        AtomicityRequirement::BestEffort,
        AtomicityRequirement::default(),
    );
}
