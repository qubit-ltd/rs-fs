use super::{FsOwnedReadRequest, FsSubmitFailure, PreparedFsReadAdapter};
use crate::spi::ManagedFsReadSpi;
use crate::{FsOperation, FsOperationFailure};
use qubit_operation::{OperationManager, OperationOptions};
use std::sync::Arc;
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
            .try_submit_external::<Vec<u8>, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            );
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
            .try_submit_external::<Vec<u8>, FsOperationFailure, _>(
                OperationOptions::default(),
                Arc::clone(&adapter),
            ) {
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
