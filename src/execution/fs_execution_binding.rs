// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use qubit_operation::GroupBindingError;
use qubit_operation::OperationGroup;
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
///
/// The binding submits provider-prepared requests to an application-owned
/// operation manager and returns rejected requests to the caller for recovery
/// or retry.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsExecutionBinding;
/// use qubit_operation::OperationManager;
///
/// let manager = OperationManager::new(Default::default()).expect("valid manager options");
/// let binding = FsExecutionBinding::new(manager);
/// assert!(std::ptr::eq(binding.manager(), binding.manager()));
/// ```
#[derive(Clone)]
pub struct FsExecutionBinding {
    /// Manager view that supplies the root or group-scoped submission target.
    manager: OperationManager,
}
impl FsExecutionBinding {
    /// Creates a binding from an application-owned operation manager view.
    ///
    /// # Parameters
    /// - `manager`: Manager view that receives submitted filesystem operations.
    ///
    /// # Returns
    /// A binding that submits through `manager`.
    #[must_use = "use the binding to submit managed filesystem operations"]
    #[inline]
    pub fn new(manager: OperationManager) -> Self {
        Self { manager }
    }
    /// Returns the manager view used for submissions.
    ///
    /// # Returns
    /// A shared reference to the operation manager held by this binding.
    #[must_use = "use the manager to submit managed filesystem operations"]
    #[inline]
    pub fn manager(&self) -> &OperationManager {
        &self.manager
    }

    /// Returns a clone bound to an additional service group while preserving
    /// this binding's root or parent submission target.
    ///
    /// # Parameters
    /// - `group`: Service group to which the cloned manager view is bound.
    ///
    /// # Returns
    /// A binding that submits to the additional group while preserving the
    /// current submission target.
    ///
    /// # Errors
    /// Returns the manager's group-binding error when `group` cannot be bound
    /// to this manager view.
    #[inline]
    pub fn with_group(&self, group: &OperationGroup) -> Result<Self, GroupBindingError> {
        self.manager.with_group(group).map(Self::new)
    }
    /// Prepares and submits a read while retaining its request on rejection.
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares the owned read request.
    /// - `request`: Owned path and read options returned if preparation or
    ///   admission rejects the operation.
    ///
    /// # Returns
    /// The submitted read operation, or a failure containing the original
    /// request and its preparation or admission reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` when provider preparation fails or the
    /// operation manager rejects admission.
    ///
    /// # Panics
    /// Panics if a rejected prepared operation no longer retains its original
    /// request, violating the provider adapter's ownership contract.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the write operation.
    /// - `request`: Owned destination, content, and write options returned on
    ///   any pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final write-effect fact.
    ///
    /// # Returns
    /// The submitted mutation operation with its effect observer, or a failure
    /// retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the copy operation.
    /// - `request`: Owned source, destination, and copy options returned on any
    ///   pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final copy-effect fact.
    ///
    /// # Returns
    /// The submitted mutation operation with its effect observer, or a failure
    /// retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the rename operation.
    /// - `request`: Owned source, destination, and rename options returned on
    ///   any pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final rename-effect fact.
    ///
    /// # Returns
    /// The submitted mutation operation with its effect observer, or a failure
    /// retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the tree publication.
    /// - `request`: Owned tree publication request returned on any
    ///   pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the publication effect fact.
    ///
    /// # Returns
    /// The submitted mutation operation with its effect observer, or a failure
    /// retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the write operation.
    /// - `request`: Owned destination, content, and write options returned on
    ///   any pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final write-effect fact.
    ///
    /// # Returns
    /// The submitted asynchronous mutation operation with its effect observer,
    /// or a failure retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the copy operation.
    /// - `request`: Owned source, destination, and copy options returned on any
    ///   pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final copy-effect fact.
    ///
    /// # Returns
    /// The submitted asynchronous mutation operation with its effect observer,
    /// or a failure retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the rename operation.
    /// - `request`: Owned source, destination, and rename options returned on
    ///   any pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the final rename-effect fact.
    ///
    /// # Returns
    /// The submitted asynchronous mutation operation with its effect observer,
    /// or a failure retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares and owns the tree publication.
    /// - `request`: Owned tree publication request returned on any
    ///   pre-submission rejection.
    /// - `pool`: Bounded pool used to reserve the publication effect fact.
    ///
    /// # Returns
    /// The submitted asynchronous mutation operation with its effect observer,
    /// or a failure retaining the original request and rejection reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` if effect capacity is unavailable, provider
    /// preparation or reservation installation fails, or manager admission
    /// rejects the operation.
    ///
    /// # Panics
    /// Panics if a prepared provider operation unexpectedly loses its request
    /// before it can be returned after rejection.
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
    ///
    /// # Parameters
    /// - `provider`: Provider that prepares the owned read request.
    /// - `request`: Owned path and read options returned if preparation or
    ///   admission rejects the operation.
    ///
    /// # Returns
    /// The submitted asynchronous read operation, or a failure containing the
    /// original request and its preparation or admission reason.
    ///
    /// # Errors
    /// Returns `FsSubmitFailure` when provider preparation fails or the
    /// asynchronous manager rejects admission.
    ///
    /// # Panics
    /// Panics if a rejected prepared operation no longer retains its original
    /// request, violating the provider adapter's ownership contract.
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
