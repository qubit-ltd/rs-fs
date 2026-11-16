use std::sync::Arc;

use qubit_operation::OperationManager;
use qubit_operation::OperationOptions;

use super::FsOwnedReadRequest;
use super::FsSubmitFailure;
use super::PreparedFsReadAdapter;
#[cfg(feature = "async")]
use crate::AsyncFsMutationOperation;
use crate::FsEffectSlotPool;
use crate::FsMutationOperation;
use crate::FsOperation;
use crate::FsOperationFailure;
use crate::FsOwnedCopyRequest;
use crate::FsOwnedRenameRequest;
use crate::FsOwnedTreePublishRequest;
use crate::FsOwnedWriteRequest;
use crate::spi::ManagedFsMutationSpi;
use crate::spi::ManagedFsReadSpi;
use crate::spi::ManagedFsTreeSpi;
/// Explicit synchronous execution binding for managed filesystem operations.
#[derive(Clone)]
pub struct FsExecutionBinding {
    manager: OperationManager,
}
impl FsExecutionBinding {
    /// Creates a binding from an application-owned operation manager view.
    pub fn new(manager: OperationManager) -> Self {
        Self { manager }
    }
    /// Returns the manager view used for submissions.
    pub fn manager(&self) -> &OperationManager {
        &self.manager
    }

    /// Returns a clone bound to an additional service group while preserving
    /// this binding's root or parent submission target.
    pub fn with_group(
        &self,
        group: &qubit_operation::OperationGroup,
    ) -> Result<Self, qubit_operation::GroupBindingError> {
        self.manager.with_group(group).map(Self::new)
    }
    /// Prepares and submits a read while retaining its request on rejection.
    pub fn submit_read_all(
        &self,
        provider: &dyn ManagedFsReadSpi,
        request: FsOwnedReadRequest,
    ) -> Result<FsOperation<Vec<u8>>, FsSubmitFailure<FsOwnedReadRequest>> {
        let prepared = match provider.prepare_read_all(request) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(failure.kind),
                    request: failure.request,
                });
            }
        };
        let adapter = Arc::new(PreparedFsReadAdapter::new(prepared));
        let result = self
            .manager
            .try_submit_external::<Vec<u8>, FsOperationFailure, _>(OperationOptions::default(), Arc::clone(&adapter));
        match result {
            Ok(operation) => Ok(operation),
            Err(rejected) => {
                let (admission, rejected_adapter) = rejected.into_parts();
                let original = rejected_adapter
                    .take_request()
                    .expect("unclaimed prepared operation retains its request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request: original,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed write.
    pub fn submit_write(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedWriteRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<FsMutationOperation<crate::metadata::WriteOutcome>, FsSubmitFailure<FsOwnedWriteRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_write(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsWriteAdapter::new(prepared));
        match self
            .manager
            .try_submit_external::<crate::metadata::WriteOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(FsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed copy.
    pub fn submit_copy(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedCopyRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<FsMutationOperation<crate::copy::CopyOutcome>, FsSubmitFailure<FsOwnedCopyRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_copy(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsCopyAdapter::new(prepared));
        match self
            .manager
            .try_submit_external::<crate::copy::CopyOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(FsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed rename.
    pub fn submit_rename(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedRenameRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<FsMutationOperation<crate::rename::RenameOutcome>, FsSubmitFailure<FsOwnedRenameRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_rename(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsRenameAdapter::new(prepared));
        match self
            .manager
            .try_submit_external::<crate::rename::RenameOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(FsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a staged-tree
    /// publication.
    pub fn submit_tree_publish(
        &self,
        provider: &dyn ManagedFsTreeSpi,
        request: FsOwnedTreePublishRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<FsMutationOperation<crate::directory::TreePublishOutcome>, FsSubmitFailure<FsOwnedTreePublishRequest>>
    {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_tree_publish(request) {
            Ok(v) => v,
            Err(failure) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(failure.kind),
                    request: failure.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed tree provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsTreePublishAdapter::new(prepared));
        match self
            .manager
            .try_submit_external::<crate::directory::TreePublishOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(FsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed tree publication retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed async
    /// write.
    #[cfg(feature = "async")]
    pub fn submit_write_async(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedWriteRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::metadata::WriteOutcome>, FsSubmitFailure<FsOwnedWriteRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_write(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsWriteAdapter::new(prepared));
        match self
            .manager
            .as_async()
            .try_submit_external::<crate::metadata::WriteOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(AsyncFsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed async
    /// copy.
    #[cfg(feature = "async")]
    pub fn submit_copy_async(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedCopyRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::copy::CopyOutcome>, FsSubmitFailure<FsOwnedCopyRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_copy(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsCopyAdapter::new(prepared));
        match self
            .manager
            .as_async()
            .try_submit_external::<crate::copy::CopyOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(AsyncFsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits a managed async
    /// rename.
    #[cfg(feature = "async")]
    pub fn submit_rename_async(
        &self,
        provider: &dyn ManagedFsMutationSpi,
        request: FsOwnedRenameRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<AsyncFsMutationOperation<crate::rename::RenameOutcome>, FsSubmitFailure<FsOwnedRenameRequest>> {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_rename(request) {
            Ok(v) => v,
            Err(f) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(f.kind),
                    request: f.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsRenameAdapter::new(prepared));
        match self
            .manager
            .as_async()
            .try_submit_external::<crate::rename::RenameOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(AsyncFsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed prepared operation retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares, reserves a final-effect slot, and submits async tree
    /// publication.
    #[cfg(feature = "async")]
    pub fn submit_tree_publish_async(
        &self,
        provider: &dyn ManagedFsTreeSpi,
        request: FsOwnedTreePublishRequest,
        pool: &FsEffectSlotPool,
    ) -> Result<
        AsyncFsMutationOperation<crate::directory::TreePublishOutcome>,
        FsSubmitFailure<FsOwnedTreePublishRequest>,
    > {
        let reservation = match pool.reserve() {
            Ok(v) => v,
            Err(_) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(super::FsPrepareFailureKind::EffectCapacity),
                    request,
                });
            }
        };
        let prepared = match provider.prepare_tree_publish(request) {
            Ok(v) => v,
            Err(failure) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(failure.kind),
                    request: failure.request,
                });
            }
        };
        let observer = reservation.observer();
        if let Err(reservation) = prepared.install_effect_reservation(reservation) {
            drop(reservation);
            return Err(FsSubmitFailure {
                admission: None,
                preparation: Some(super::FsPrepareFailureKind::ReservationInstallFailed),
                request: prepared
                    .take_unclaimed_request()
                    .expect("unclaimed tree provider work retains request"),
            });
        }
        let adapter = Arc::new(super::PreparedFsTreePublishAdapter::new(prepared));
        match self
            .manager
            .as_async()
            .try_submit_external::<crate::directory::TreePublishOutcome, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
            Ok(operation) => Ok(AsyncFsMutationOperation {
                operation,
                effect: observer,
            }),
            Err(rejected) => {
                let (admission, rejected) = rejected.into_parts();
                let request = rejected
                    .take_request()
                    .expect("unclaimed tree publication retains request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request,
                })
            }
        }
    }

    /// Prepares and submits a read through the asynchronous manager view.
    #[cfg(feature = "async")]
    pub fn submit_read_all_async(
        &self,
        provider: &dyn ManagedFsReadSpi,
        request: FsOwnedReadRequest,
    ) -> Result<crate::AsyncFsOperation<Vec<u8>>, FsSubmitFailure<FsOwnedReadRequest>> {
        let prepared = match provider.prepare_read_all(request) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(FsSubmitFailure {
                    admission: None,
                    preparation: Some(failure.kind),
                    request: failure.request,
                });
            }
        };
        let adapter = Arc::new(PreparedFsReadAdapter::new(prepared));
        match self
            .manager
            .as_async()
            .try_submit_external::<Vec<u8>, FsOperationFailure, _>(OperationOptions::default(), Arc::clone(&adapter))
        {
            Ok(operation) => Ok(operation),
            Err(rejected) => {
                let (admission, rejected_adapter) = rejected.into_parts();
                let original = rejected_adapter
                    .take_request()
                    .expect("unclaimed prepared operation retains its request");
                Err(FsSubmitFailure {
                    admission: Some(admission),
                    preparation: None,
                    request: original,
                })
            }
        }
    }
}
