// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider preparation and staging contract for tree publication.

use std::sync::Arc;

use qubit_operation::ResourceCapacityError;
use qubit_operation::ResourceRef;
use qubit_operation::ResourceReservation;
use qubit_operation::ResourceScope;

use super::PreparedFsTreePublish;
use super::internal::StagedDirectoryCleanup;
use super::staged_directory::StagedCleanupCallback;
use super::staged_directory::StagedDirectory;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::execution::FsOwnedTreePublishRequest;
use crate::execution::FsPrepareFailure;
use crate::metadata::TreeValidationOptions;
use crate::path::Path;

/// Object-safe provider preparation interface for staged-tree publication.
///
/// Providers validate staged content and publication requirements before
/// activating I/O. Unsupported `Required` guarantees fail during preparation.
/// Required atomic publication compares the request's validated version in
/// the publication transaction; a preflight-only check is insufficient.
/// Validation failures leave cleanup with the staging owner, while successful
/// publication reports achieved atomicity and durability in
/// [`crate::directory::TreePublishOutcome`].
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::ManagedFsTreeSpi;
/// use qubit_operation::ResourceScope;
///
/// fn reserve_staging<T: ManagedFsTreeSpi>(provider: &T, scope: &ResourceScope) {
///     assert!(provider.reserve_staged_directory(scope).is_ok());
/// }
/// ```
pub trait ManagedFsTreeSpi: Send + Sync {
    /// Reserves operation-scope capacity before provider staging begins.
    ///
    /// # Parameters
    /// - `scope`: Operation resource scope that will own staging cleanup.
    ///
    /// # Returns
    /// A reservation that must be committed or released before staging starts.
    ///
    /// # Errors
    /// Returns a capacity error when the scope cannot reserve another resource.
    #[inline]
    fn reserve_staged_directory(&self, scope: &ResourceScope) -> Result<ResourceReservation, ResourceCapacityError> {
        scope.reserve_resource()
    }

    /// Commits the staged owner and async cleanup policy into the reservation.
    ///
    /// The caller creates provider staging only after
    /// [`reserve_staged_directory`](Self::reserve_staged_directory) succeeds.
    /// If commit fails, the rejected value and callback are routed through the
    /// reservation's cleanup path before this method returns an error.
    ///
    /// # Parameters
    /// - `reservation`: Capacity reserved before provider staging began.
    /// - `root`: Logical root of the staged tree.
    /// - `options`: Tree validation requirements and limits.
    /// - `cleanup`: Async callback that releases provider staging resources.
    ///
    /// # Returns
    /// A resource-scope reference to the registered staging owner.
    ///
    /// # Errors
    /// Returns an error if resource registration fails; the staging owner and
    /// cleanup responsibility remain retained for recovery.
    fn commit_staged_directory(
        &self,
        reservation: ResourceReservation,
        root: Path,
        options: TreeValidationOptions,
        cleanup: StagedCleanupCallback,
    ) -> FsResult<ResourceRef<StagedDirectory>> {
        let owner = StagedDirectory::new(root, options, cleanup);
        match reservation.commit_with(owner, StagedDirectoryCleanup) {
            Ok(resource) => {
                resource.set_cleanup_resource_id(resource.id());
                Ok(resource)
            }
            Err(error) => {
                let (reason, owner, cleanup, reservation) = error.into_parts();
                let resource_id = reservation.retain_with_for_cleanup(owner, cleanup);
                let message =
                    format!("staging owner registration failed ({reason:?}); cleanup retained as {resource_id:?}");
                Err(FsError::new(
                    FsErrorKind::InvalidState,
                    FsOperation::CreateTemp,
                    &message,
                ))
            }
        }
    }

    /// Prepares publication without starting tree I/O.
    ///
    /// Implementations must reject unsupported required guarantees before
    /// activating any I/O. They must not infer atomic tree publication from
    /// an object-prefix operation. For required atomic publication, they must
    /// compare `request.validated_version` in the same transaction that
    /// publishes the destination, or hold an equivalent lock through publish.
    /// If the provider cannot do so, it rejects during preparation before
    /// side effects. Any validation failure leaves cleanup with the existing
    /// staging owner.
    ///
    /// # Parameters
    /// - `request`: Validated owned tree-publication request.
    ///
    /// # Returns
    /// A prepared tree publication operation on success.
    ///
    /// # Errors
    /// Returns the preparation failure together with the original request.
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>>;
}
