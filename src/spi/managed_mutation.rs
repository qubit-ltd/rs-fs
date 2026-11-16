//! Owned mutation protocol declarations for managed providers.
use crate::{
    copy::CopyOutcome, metadata::WriteOutcome, rename::RenameOutcome, FsOperationFailure,
    execution::{FsOwnedCopyRequest, FsOwnedRenameRequest, FsOwnedWriteRequest, FsPrepareFailure},
};
use qubit_operation::PreparedExternalOperation;
use std::sync::Arc;
/// Object-safe prepared write operation with request recovery before claim.
pub trait PreparedFsWrite: PreparedExternalOperation<WriteOutcome, FsOperationFailure> {
    /// Returns the original write request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedWriteRequest>;
}
/// Object-safe prepared copy operation with request recovery before claim.
pub trait PreparedFsCopy: PreparedExternalOperation<CopyOutcome, FsOperationFailure> {
    /// Returns the original copy request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedCopyRequest>;
}
/// Object-safe prepared rename operation with request recovery before claim.
pub trait PreparedFsRename: PreparedExternalOperation<RenameOutcome, FsOperationFailure> {
    /// Returns the original rename request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedRenameRequest>;
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
