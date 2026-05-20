use qubit_fs::PathSemantics;

#[test]
fn test_path_semantics_hierarchical_variant_is_comparable() {
    let semantics = PathSemantics::Hierarchical;

    assert!(matches!(semantics, PathSemantics::Hierarchical));
}
