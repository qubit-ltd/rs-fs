#[test]
fn test_required_non_atomic_temp_persist_retains_cleanup_responsibility() {
    let (filesystem, cleanup_calls, _) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(qubit_fs::TempFileOptions::default())
        .expect("temporary file should open");
    let error = temporary
        .persist(
            &qubit_fs::Path::parse("/target").expect("target should parse"),
            qubit_fs::PersistOptions::default(),
        )
        .expect_err("non-atomic result must violate required contract");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.error().kind()
    );
    assert_eq!(
        qubit_fs::TempResourceState::CleanupRequired,
        temporary.state()
    );
    temporary
        .cleanup()
        .expect("cleanup should remain available");
    assert_eq!(
        1,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}

#[test]
fn test_temp_file_illegal_target_fails_preflight_without_provider_persist_and_remains_owned()
 {
    let (filesystem, cleanup_calls, persist_calls) =
        crate::handle_support::filesystem(false, Vec::new());
    let mut temporary = filesystem
        .create_temp_file(qubit_fs::TempFileOptions::default())
        .expect("temporary file should open");
    let error = temporary
        .persist(
            &qubit_fs::Path::parse("relative")
                .expect("relative path should parse"),
            qubit_fs::PersistOptions::default(),
        )
        .expect_err("illegal target must fail before provider persist");
    assert_eq!(qubit_fs::FsErrorKind::InvalidPath, error.error().kind());
    assert_eq!(qubit_fs::PersistFailureState::NotPublished, error.state());
    assert_eq!(qubit_fs::TempResourceState::Owned, temporary.state());
    assert_eq!(
        0,
        *persist_calls.lock().expect("persist lock should succeed")
    );
    temporary
        .cleanup()
        .expect("owned resource should remain recoverable");
    assert_eq!(
        1,
        *cleanup_calls.lock().expect("cleanup lock should succeed")
    );
}

#[test]
fn test_temp_file_rejects_provider_path_outside_facade_constraints() {
    let error = crate::handle_support::invalid_temp_path_filesystem()
        .create_temp_file(qubit_fs::TempFileOptions::default())
        .expect_err("relative provider temporary path must be rejected");
    assert_eq!(
        qubit_fs::FsErrorKind::ProviderContractViolation,
        error.kind()
    );
}
