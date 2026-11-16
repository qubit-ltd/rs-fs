use qubit_fs::{
    FsEffectFact, FsEffectSlotPool, FsEffectSnapshot, FsEffectState, FsExecutionBinding,
    FsOwnedReadRequest, FsPrepareFailure, FsPrepareFailureKind, Path,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

struct RejectReadProvider {
    prepared: AtomicUsize,
    io_started: AtomicUsize,
}
impl qubit_fs::spi::ManagedFsReadSpi for RejectReadProvider {
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn qubit_fs::spi::PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>> {
        self.prepared.fetch_add(1, Ordering::SeqCst);
        Err(FsPrepareFailure {
            kind: FsPrepareFailureKind::Rejected("policy".into()),
            request,
        })
    }
}

#[test]
fn parent_scope_submission_preserves_request_when_prepare_rejects_without_io() {
    let manager = qubit_operation::OperationManager::new(Default::default()).unwrap();
    let provider = Arc::new(RejectReadProvider {
        prepared: AtomicUsize::new(0),
        io_started: AtomicUsize::new(0),
    });
    let path = Path::parse("/managed/input").unwrap();
    let expected = path.clone();
    let mut child_parent = manager
        .try_submit(Default::default(), {
            let provider = Arc::clone(&provider);
            move |scope| {
                let binding = FsExecutionBinding::new(scope.manager());
                let error = match binding.submit_read_all(
                    provider.as_ref(),
                    FsOwnedReadRequest {
                        path,
                        options: Default::default(),
                    },
                ) {
                    Ok(_) => panic!("provider rejected preparation"),
                    Err(error) => error,
                };
                let (_, preparation, request) = error.into_parts();
                assert_eq!(request.path, expected);
                assert_eq!(
                    preparation,
                    Some(FsPrepareFailureKind::Rejected("policy".into()))
                );
                Ok::<_, ()>(())
            }
        })
        .unwrap();
    assert!(matches!(
        child_parent.wait_outcome().unwrap(),
        qubit_operation::OperationOutcome::Succeeded(())
    ));
    assert_eq!(provider.prepared.load(Ordering::SeqCst), 1);
    assert_eq!(provider.io_started.load(Ordering::SeqCst), 0);
}

#[test]
fn effect_slot_is_reserved_until_provider_releases_it() {
    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("first slot is available");
    let observer = reservation.observer();
    assert!(matches!(observer.snapshot(), FsEffectSnapshot::Pending));
    assert!(pool.reserve().is_err());

    reservation.publish(FsEffectFact::State(FsEffectState::Applied));
    assert!(matches!(
        observer.snapshot(),
        FsEffectSnapshot::Final(FsEffectFact::State(FsEffectState::Applied))
    ));
    drop(reservation);
    assert!(pool.reserve().is_ok());
}

#[test]
fn dropping_observer_does_not_release_provider_resource_owner() {
    struct RecoveryOwner(qubit_fs::FsEffectReservation);
    impl RecoveryOwner {
        fn finish_recovery(self) {
            drop(self.0);
        }
    }

    let pool = FsEffectSlotPool::new(1);
    let reservation = pool.reserve().expect("capacity available");
    let observer = reservation.observer();
    let owner = RecoveryOwner(reservation);
    drop(observer);
    assert!(
        pool.reserve().is_err(),
        "observer is not the resource owner"
    );
    owner.finish_recovery();
    assert!(
        pool.reserve().is_ok(),
        "recovery owner releases its lease at completion"
    );
}
