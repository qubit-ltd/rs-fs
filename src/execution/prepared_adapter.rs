use crate::{FsOperationFailure, execution::FsOwnedReadRequest, spi::PreparedFsRead};
use qubit_operation::{
    ExecutionFailure, ExternalBindError, OperationControl, OperationId, OperationOutcome,
    OperationScope, PreparedExternalOperation,
};
use std::{
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
/// Sized wrapper adapting an object-safe prepared read to the operation API.
pub struct PreparedFsReadAdapter {
    prepared: Arc<dyn PreparedFsRead>,
    lifecycle: Mutex<PreparedReadLifecycle>,
}

#[derive(Default)]
struct PreparedReadLifecycle {
    bound: Option<OperationId>,
    activated: bool,
    abandoned: bool,
}
impl PreparedFsReadAdapter {
    /// Creates an adapter retaining its request until submission.
    pub fn new(prepared: Arc<dyn PreparedFsRead>) -> Self {
        Self {
            prepared,
            lifecycle: Mutex::new(PreparedReadLifecycle::default()),
        }
    }
    /// Takes the request when submission has been rejected before activation.
    pub fn take_request(&self) -> Option<FsOwnedReadRequest> {
        let is_unclaimed = self
            .lifecycle
            .lock()
            .map(|state| state.bound.is_none())
            .unwrap_or(false);
        is_unclaimed
            .then(|| self.prepared.take_unclaimed_request())
            .flatten()
    }
}
impl OperationControl for PreparedFsReadAdapter {
    fn request_stop(&self, r: qubit_operation::StopReason) {
        self.prepared.request_stop(r);
    }
    fn snapshot(&self) -> qubit_operation::ChildSnapshot {
        self.prepared.snapshot()
    }
    fn poll_outcome(&self, c: &mut Context<'_>) -> Poll<qubit_operation::ChildOutcomeSummary> {
        self.prepared.poll_outcome(c)
    }
    fn poll_release(&self, c: &mut Context<'_>) -> Poll<qubit_operation::ChildRelease> {
        self.prepared.poll_release(c)
    }
    fn poll_settlement(&self, c: &mut Context<'_>) -> Poll<Arc<qubit_operation::SettlementReport>> {
        self.prepared.poll_settlement(c)
    }
}
impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for PreparedFsReadAdapter {
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        let mut state = self
            .lifecycle
            .lock()
            .map_err(|_| ExternalBindError::AlreadyBound)?;
        if state.bound.is_some() || state.abandoned || state.activated {
            return Err(ExternalBindError::AlreadyBound);
        }
        self.prepared.bind(id)?;
        state.bound = Some(id);
        Ok(())
    }
    fn activate(self: Arc<Self>, scope: OperationScope) -> Result<(), ExecutionFailure> {
        {
            let mut state = self
                .lifecycle
                .lock()
                .expect("prepared read lifecycle mutex poisoned");
            if state.activated || state.abandoned {
                if let Some(operation_id) = state.bound {
                    return Err(ExecutionFailure {
                        operation_id,
                        summary: qubit_operation::DiagnosticSummary::new(
                            "prepared read was already activated or abandoned",
                        ),
                        source: None,
                    });
                }
            }
            state.activated = true;
        }
        Arc::clone(&self.prepared).activate(scope)
    }
    fn abandon(self: Arc<Self>) {
        let should_abandon = self
            .lifecycle
            .lock()
            .map(|mut state| {
                if state.bound.is_some() && !state.activated && !state.abandoned {
                    state.abandoned = true;
                    true
                } else {
                    false
                }
            })
            .unwrap_or(false);
        if should_abandon {
            Arc::clone(&self.prepared).abandon();
        }
    }
    fn poll_take_outcome(
        &self,
        c: &mut Context<'_>,
    ) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        self.prepared.poll_take_outcome(c)
    }
}

macro_rules! prepared_adapter {
    ($name:ident, $prepared:ident, $request:ty, $output:ty) => {
        /// Sized adapter for the corresponding object-safe prepared provider task.
        pub struct $name {
            prepared: Arc<dyn crate::spi::$prepared>,
            lifecycle: Mutex<PreparedReadLifecycle>,
        }
        impl $name {
            /// Creates an adapter for an unclaimed provider task.
            pub fn new(prepared: Arc<dyn crate::spi::$prepared>) -> Self {
                Self {
                    prepared,
                    lifecycle: Mutex::new(PreparedReadLifecycle::default()),
                }
            }
            /// Returns the original request while the task remains unclaimed.
            pub fn take_request(&self) -> Option<$request> {
                let unclaimed = self
                    .lifecycle
                    .lock()
                    .map(|state| state.bound.is_none())
                    .unwrap_or(false);
                unclaimed
                    .then(|| self.prepared.take_unclaimed_request())
                    .flatten()
            }
        }
        impl OperationControl for $name {
            fn request_stop(&self, reason: qubit_operation::StopReason) {
                self.prepared.request_stop(reason);
            }
            fn snapshot(&self) -> qubit_operation::ChildSnapshot {
                self.prepared.snapshot()
            }
            fn poll_outcome(
                &self,
                cx: &mut Context<'_>,
            ) -> Poll<qubit_operation::ChildOutcomeSummary> {
                self.prepared.poll_outcome(cx)
            }
            fn poll_release(&self, cx: &mut Context<'_>) -> Poll<qubit_operation::ChildRelease> {
                self.prepared.poll_release(cx)
            }
            fn poll_settlement(
                &self,
                cx: &mut Context<'_>,
            ) -> Poll<Arc<qubit_operation::SettlementReport>> {
                self.prepared.poll_settlement(cx)
            }
        }
        impl PreparedExternalOperation<$output, FsOperationFailure> for $name {
            fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
                let mut state = self
                    .lifecycle
                    .lock()
                    .map_err(|_| ExternalBindError::AlreadyBound)?;
                if state.bound.is_some() || state.activated || state.abandoned {
                    return Err(ExternalBindError::AlreadyBound);
                }
                self.prepared.bind(id)?;
                state.bound = Some(id);
                Ok(())
            }
            fn activate(self: Arc<Self>, scope: OperationScope) -> Result<(), ExecutionFailure> {
                let mut state = self
                    .lifecycle
                    .lock()
                    .expect("prepared operation lifecycle mutex poisoned");
                if state.activated || state.abandoned {
                    if let Some(operation_id) = state.bound {
                        return Err(ExecutionFailure {
                            operation_id,
                            summary: qubit_operation::DiagnosticSummary::new(
                                "prepared operation was already activated or abandoned",
                            ),
                            source: None,
                        });
                    }
                }
                state.activated = true;
                drop(state);
                Arc::clone(&self.prepared).activate(scope)
            }
            fn abandon(self: Arc<Self>) {
                let should_abandon = self
                    .lifecycle
                    .lock()
                    .map(|mut state| {
                        if state.bound.is_some() && !state.activated && !state.abandoned {
                            state.abandoned = true;
                            true
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);
                if should_abandon {
                    Arc::clone(&self.prepared).abandon();
                }
            }
            fn poll_take_outcome(
                &self,
                cx: &mut Context<'_>,
            ) -> Poll<OperationOutcome<$output, FsOperationFailure>> {
                self.prepared.poll_take_outcome(cx)
            }
        }
    };
}

prepared_adapter!(
    PreparedFsWriteAdapter,
    PreparedFsWrite,
    crate::execution::FsOwnedWriteRequest,
    crate::metadata::WriteOutcome
);
prepared_adapter!(
    PreparedFsCopyAdapter,
    PreparedFsCopy,
    crate::execution::FsOwnedCopyRequest,
    crate::copy::CopyOutcome
);
prepared_adapter!(
    PreparedFsRenameAdapter,
    PreparedFsRename,
    crate::execution::FsOwnedRenameRequest,
    crate::rename::RenameOutcome
);
prepared_adapter!(
    PreparedFsTreePublishAdapter,
    PreparedFsTreePublish,
    crate::execution::FsOwnedTreePublishRequest,
    crate::directory::TreePublishOutcome
);
