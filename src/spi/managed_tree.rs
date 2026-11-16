//! Owned tree-operation protocol declarations for managed providers.
use crate::{
    directory::TreePublishOutcome,
    FsOperationFailure,
    execution::{FsOwnedTreePublishRequest, FsPrepareFailure},
};
use qubit_operation::PreparedExternalOperation;
use std::sync::Arc;
/// Object-safe prepared staged-tree publication operation.
pub trait PreparedFsTreePublish: PreparedExternalOperation<TreePublishOutcome, FsOperationFailure> {
    /// Returns the original tree request while unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedTreePublishRequest>;
}
/// Object-safe provider preparation interface for staged-tree publication.
pub trait ManagedFsTreeSpi: Send + Sync {
    /// Prepares publication without starting tree I/O.
    fn prepare_tree_publish(
        &self,
        request: FsOwnedTreePublishRequest,
    ) -> Result<Arc<dyn PreparedFsTreePublish>, FsPrepareFailure<FsOwnedTreePublishRequest>>;
}
