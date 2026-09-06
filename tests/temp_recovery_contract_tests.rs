use qubit_fs::error::FsError;
use qubit_fs::error::FsErrorKind;
use qubit_fs::error::FsOperation;
use qubit_fs::temp::PersistFailure;
use qubit_fs::temp::PersistFailureState;
#[test]
fn test_persist_failure_retains_release_state() {
    let failure = PersistFailure::new(
        FsError::new(FsErrorKind::Io, FsOperation::PersistTemp, "injected"),
        PersistFailureState::PublishedSourceReleased,
    );
    assert_eq!(None, failure.publication_target());
    let (_, state, restored_target) = failure.into_recovery_parts();
    assert_eq!(PersistFailureState::PublishedSourceReleased, state);
    assert_eq!(None, restored_target);
}
