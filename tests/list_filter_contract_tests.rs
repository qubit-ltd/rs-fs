use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::path::PathSemantics;

#[test]
fn test_literal_prefix_preserves_raw_text_for_object_keys() {
    let filter = ListFilter::LiteralPrefix("a/../b//%2F".to_owned());
    let options = ListOptions::object_keys().with_filter(Some(filter.clone()));
    assert!(options.validate_for(PathSemantics::ObjectKey).is_ok());
    assert_eq!(Some(&filter), options.filter());
}

#[test]
fn test_subtree_remains_hierarchical_and_legacy_prefix_compatible() {
    let options = ListOptions::default().with_prefix(Some("nested/item".to_owned()));
    assert_eq!(Some("nested/item"), options.prefix());
    assert!(options.validate_for(PathSemantics::Hierarchical).is_ok());
}
