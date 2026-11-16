//! Object-safe owned read provider protocol.
use crate::FsOperationFailure;
use crate::execution::{FsOwnedReadRequest, FsPrepareFailure};
use std::sync::Arc;
/// Prepared read accepted by the managed operation framework.
pub trait PreparedFsRead:
    qubit_operation::PreparedExternalOperation<Vec<u8>, FsOperationFailure>
{
    /// Returns the original request while the prepared operation remains unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest>;
}
/// Object-safe provider preparation interface for owned read requests.
pub trait ManagedFsReadSpi: Send + Sync {
    /// Reserves provider resources and prepares a read without starting I/O.
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>>;
}
