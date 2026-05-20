use qubit_fs::CredentialRef;

#[test]
fn test_credential_ref_variants_are_constructible_and_comparable() {
    assert_eq!(
        CredentialRef::Profile("p".to_owned()),
        CredentialRef::Profile("p".to_owned()),
    );
    assert_eq!(
        CredentialRef::Environment {
            access_key: "AK".to_owned(),
            secret_key: "SK".to_owned(),
        },
        CredentialRef::Environment {
            access_key: "AK".to_owned(),
            secret_key: "SK".to_owned(),
        },
    );
    assert_eq!(
        CredentialRef::Provider("vault".to_owned()),
        CredentialRef::Provider("vault".to_owned()),
    );
}
