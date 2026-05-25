use qubit_fs::ServerSidePreference;

#[test]
fn test_server_side_preference_default_is_prefer() {
    assert_eq!(ServerSidePreference::Prefer, ServerSidePreference::default());
}
