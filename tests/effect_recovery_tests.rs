use qubit_fs::error::FsEffectState;
use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;

#[test]
fn test_io_error_with_indeterminate_effect_is_uncertain() {
    let error = FsError::new(FsErrorKind::Io, FsOperation::CleanupTemp, "injected")
        .with_effect_state(FsEffectState::Indeterminate);
    assert!(error.has_indeterminate_effect());
    assert_eq!(FsErrorKind::Io, error.kind());
}

#[test]
fn test_legacy_indeterminate_kind_remains_conservative() {
    let error = FsError::new(FsErrorKind::Indeterminate, FsOperation::AbortWriter, "injected")
        .with_effect_state(FsEffectState::Unchanged);
    assert!(error.has_indeterminate_effect());
}
