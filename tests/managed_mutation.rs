use qubit_fs::{FsEffectFact, FsEffectSlotPool, FsEffectSnapshot, FsEffectState};

#[test]
fn effect_reservation_stays_owned_until_recovery_finishes() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("slot is initially available");
    let observer = reservation.observer();
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(pool.reserve().is_err());

    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(observer);
    assert!(
        pool.reserve().is_err(),
        "active owner retains the bounded slot"
    );
    drop(reservation);
    assert!(
        pool.reserve().is_ok(),
        "releasing the owner reclaims the slot"
    );
}

#[test]
fn prepared_mutations_accept_one_owned_effect_reservation() {
    use qubit_fs::spi::{PreparedFsCopy, PreparedFsRename, PreparedFsWrite};

    let _: fn(
        &dyn PreparedFsWrite,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsWrite::install_effect_reservation;
    let _: fn(
        &dyn PreparedFsCopy,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsCopy::install_effect_reservation;
    let _: fn(
        &dyn PreparedFsRename,
        qubit_fs::execution::FsEffectReservation,
    ) -> Result<(), qubit_fs::execution::FsEffectReservation> =
        PreparedFsRename::install_effect_reservation;
}

#[test]
fn a_lost_effect_is_not_reported_as_unchanged() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish(FsEffectFact::State(FsEffectState::Indeterminate));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Indeterminate))
    ));
}

#[test]
fn failure_publication_preserves_write_count_and_publication_state() {
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use qubit_fs::write::WriteFailureState;
    use std::sync::Arc;

    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_write_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Write,
            "write failed",
        )),
        WriteFailureState::Published,
        23,
        None,
    );
    let FsEffectSnapshot::Final(FsEffectFact::WriteFailure {
        state,
        written_bytes,
        recovery,
        ..
    }) = observer.snapshot()
    else {
        panic!("typed write failure fact expected");
    };
    assert_eq!(WriteFailureState::Published, state);
    assert_eq!(23, written_bytes);
    assert_eq!(None, recovery);
}

#[test]
fn failure_publication_preserves_copy_counters_and_uncertainty() {
    use qubit_fs::copy::{CopyFailureState, CopyStats};
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use std::sync::Arc;

    let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_copy_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Copy,
            "copy failed",
        )),
        CopyFailureState::Indeterminate,
        CopyStats {
            files: 2,
            bytes: 91,
            ..CopyStats::default()
        },
        None,
    );
    let FsEffectSnapshot::Final(FsEffectFact::CopyFailure {
        state,
        partial_stats,
        recovery,
        ..
    }) = observer.snapshot()
    else {
        panic!("typed copy failure fact expected");
    };
    assert_eq!(CopyFailureState::Indeterminate, state);
    assert_eq!(2, partial_stats.files);
    assert_eq!(91, partial_stats.bytes);
    assert_eq!(None, recovery);
}

#[test]
fn failure_publication_preserves_rename_transition_state() {
    use qubit_fs::error::{FsError, FsErrorKind, FsOperation};
    use qubit_fs::rename::RenameFailureState;
    use std::sync::Arc;

    let reservation = FsEffectSlotPool::new(1).reserve().unwrap();
    let observer = reservation.observer();
    reservation.publish_rename_failure(
        Arc::new(FsError::new(
            FsErrorKind::Other,
            FsOperation::Rename,
            "rename failed",
        )),
        RenameFailureState::Renamed,
    );
    let FsEffectSnapshot::Final(FsEffectFact::RenameFailure { state, .. }) = observer.snapshot()
    else {
        panic!("typed rename failure fact expected");
    };
    assert_eq!(RenameFailureState::Renamed, state);
}

#[test]
fn content_and_version_conditions_are_distinct_inputs() {
    use qubit_fs::metadata::{ContentPrecondition, ResourceVersion};
    use qubit_fs::write::{WriteOptions, WritePrecondition};

    let options = WriteOptions::default().with_precondition(WritePrecondition::IfMatch(
        ResourceVersion::new("revision-7"),
    ));
    let content = ContentPrecondition::Equals(b"expected bytes".to_vec());
    assert_eq!(
        &WritePrecondition::IfMatch(ResourceVersion::new("revision-7")),
        options.precondition()
    );
    assert_eq!(
        ContentPrecondition::Equals(b"expected bytes".to_vec()),
        content
    );
}

#[test]
fn write_content_condition_is_preserved_independently_from_version_condition() {
    use qubit_fs::metadata::{ContentPrecondition, ResourceVersion};
    use qubit_fs::write::{WriteOptions, WritePrecondition};

    let options = WriteOptions::default()
        .with_precondition(WritePrecondition::IfMatch(ResourceVersion::new("v4")))
        .with_content_precondition(ContentPrecondition::Equals(b"old bytes".to_vec()));
    assert_eq!(
        &WritePrecondition::IfMatch(ResourceVersion::new("v4")),
        options.precondition()
    );
    assert_eq!(
        &ContentPrecondition::Equals(b"old bytes".to_vec()),
        options.content_precondition()
    );
}

#[test]
fn required_content_compare_needs_guaranteed_conditional_write() {
    use qubit_fs::metadata::{FileSystemCapabilities, FileSystemCapability};
    use qubit_fs::write::WriteOptions;

    let options = WriteOptions::default()
        .with_content_precondition(qubit_fs::metadata::ContentPrecondition::Equals(vec![1]));
    let conditional = FileSystemCapabilities::new()
        .with_conditional(FileSystemCapability::Write)
        .with_conditional(FileSystemCapability::ConditionalWrite);
    assert!(options.validate_against(conditional).is_err());
    let guaranteed = FileSystemCapabilities::new()
        .with_guaranteed(FileSystemCapability::Write)
        .with_guaranteed(FileSystemCapability::ConditionalWrite);
    assert!(options.validate_against(guaranteed).is_ok());
}
