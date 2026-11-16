//! Owned mutation protocol declarations for managed providers.
use crate::{
    FsOperationFailure,
    copy::CopyOutcome,
    execution::{FsOwnedCopyRequest, FsOwnedRenameRequest, FsOwnedWriteRequest, FsPrepareFailure},
    metadata::WriteOutcome,
    rename::RenameOutcome,
};
use qubit_operation::PreparedExternalOperation;
use std::sync::Arc;
/// Object-safe prepared write operation with request recovery before claim.
pub trait PreparedFsWrite: PreparedExternalOperation<WriteOutcome, FsOperationFailure> {
    /// Returns the original write request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedWriteRequest>;

    /// Installs the single bounded effect reservation before submission.
    ///
    /// Implementations synchronize this one-shot slot: exactly one concurrent
    /// installation may succeed. The accepted reservation stays with the
    /// active work owner until it publishes one final typed fact and recovery
    /// responsibility is released. A duplicate or late installation returns
    /// the supplied reservation unchanged.
    fn install_effect_reservation(
        &self,
        reservation: crate::execution::FsEffectReservation,
    ) -> Result<(), crate::execution::FsEffectReservation>;
}
/// Object-safe prepared copy operation with request recovery before claim.
pub trait PreparedFsCopy: PreparedExternalOperation<CopyOutcome, FsOperationFailure> {
    /// Returns the original copy request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedCopyRequest>;

    /// Installs the single bounded effect reservation before submission.
    ///
    /// Implementations synchronize this one-shot slot: exactly one concurrent
    /// installation may succeed. The accepted reservation stays with the
    /// active work owner until it publishes one final typed fact and recovery
    /// responsibility is released. A duplicate or late installation returns
    /// the supplied reservation unchanged.
    fn install_effect_reservation(
        &self,
        reservation: crate::execution::FsEffectReservation,
    ) -> Result<(), crate::execution::FsEffectReservation>;
}
/// Object-safe prepared rename operation with request recovery before claim.
pub trait PreparedFsRename: PreparedExternalOperation<RenameOutcome, FsOperationFailure> {
    /// Returns the original rename request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedRenameRequest>;

    /// Installs the single bounded effect reservation before submission.
    ///
    /// Implementations synchronize this one-shot slot: exactly one concurrent
    /// installation may succeed. The accepted reservation stays with the
    /// active work owner until it publishes one final typed fact and recovery
    /// responsibility is released. A duplicate or late installation returns
    /// the supplied reservation unchanged.
    fn install_effect_reservation(
        &self,
        reservation: crate::execution::FsEffectReservation,
    ) -> Result<(), crate::execution::FsEffectReservation>;
}
/// Object-safe provider preparation interface for owned mutations.
pub trait ManagedFsMutationSpi: Send + Sync {
    /// Prepares a write without starting I/O.
    fn prepare_write(
        &self,
        request: FsOwnedWriteRequest,
    ) -> Result<Arc<dyn PreparedFsWrite>, FsPrepareFailure<FsOwnedWriteRequest>>;
    /// Prepares a copy without starting I/O.
    fn prepare_copy(
        &self,
        request: FsOwnedCopyRequest,
    ) -> Result<Arc<dyn PreparedFsCopy>, FsPrepareFailure<FsOwnedCopyRequest>>;
    /// Prepares a rename without starting I/O.
    fn prepare_rename(
        &self,
        request: FsOwnedRenameRequest,
    ) -> Result<Arc<dyn PreparedFsRename>, FsPrepareFailure<FsOwnedRenameRequest>>;
}

impl crate::execution::FsEffectReservation {
    /// Publishes typed write failure facts without taking the write outcome or
    /// retaining the writer that owns recovery responsibility.
    pub fn publish_write_failure(
        &self,
        error: Arc<crate::FsError>,
        state: crate::write::WriteFailureState,
        written_bytes: u64,
        recovery: Option<qubit_operation::RecoveryId>,
    ) {
        self.publish(crate::execution::FsEffectFact::WriteFailure {
            error,
            state,
            written_bytes,
            recovery,
        });
    }

    /// Publishes typed copy failure facts without taking the copy outcome or
    /// retaining a writer that owns recovery responsibility.
    pub fn publish_copy_failure(
        &self,
        error: Arc<crate::FsError>,
        state: crate::copy::CopyFailureState,
        partial_stats: crate::copy::CopyStats,
        recovery: Option<qubit_operation::RecoveryId>,
    ) {
        self.publish(crate::execution::FsEffectFact::CopyFailure {
            error,
            state,
            partial_stats,
            recovery,
        });
    }

    /// Publishes typed rename failure facts independently of its outcome.
    pub fn publish_rename_failure(
        &self,
        error: Arc<crate::FsError>,
        state: crate::rename::RenameFailureState,
    ) {
        self.publish(crate::execution::FsEffectFact::RenameFailure { error, state });
    }
}
