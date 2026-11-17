// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Context;
use std::task::Poll;

use qubit_operation::ChildOutcomeSummary;
use qubit_operation::ChildRelease;
use qubit_operation::ChildSnapshot;
use qubit_operation::DiagnosticSummary;
use qubit_operation::ExecutionFailure;
use qubit_operation::ExternalBindError;
use qubit_operation::OperationControl;
use qubit_operation::OperationId;
use qubit_operation::OperationOutcome;
use qubit_operation::OperationScope;
use qubit_operation::PreparedExternalOperation;
use qubit_operation::SettlementReport;
use qubit_operation::StopReason;

use crate::FsOperationFailure;
use crate::execution::FsOwnedReadRequest;
use crate::execution::internal::PreparedReadLifecycle;
use crate::spi::PreparedFsRead;

/// Adapts an object-safe prepared read task to the managed operation API.
///
/// The adapter retains the provider task and its request until binding,
/// activation, or abandonment determines ownership of that request.
///
/// # Examples
///
/// A provider supplies the prepared task before submission; the caller can
/// recover its request if submission is rejected before the task is bound.
///
/// ```
/// use std::sync::Arc;
/// use qubit_fs::execution::PreparedFsReadAdapter;
/// use qubit_fs::spi::PreparedFsRead;
///
/// fn recover_unsubmitted_request(prepared: Arc<dyn PreparedFsRead>) {
///     let adapter = PreparedFsReadAdapter::new(prepared);
///     if let Some(request) = adapter.take_request() {
///         drop(request);
///     }
/// }
/// ```
#[must_use = "retain the adapter until its provider task is bound or abandoned"]
pub struct PreparedFsReadAdapter {
    /// Object-safe provider task retained until binding or abandonment.
    prepared: Arc<dyn PreparedFsRead>,
    /// Binding and activation state used to guard request recovery.
    lifecycle: Mutex<PreparedReadLifecycle>,
}

impl PreparedFsReadAdapter {
    /// Wraps a provider task so it can participate in managed read execution.
    ///
    /// # Parameters
    /// - `prepared`: Provider task that owns the read request before
    ///   submission.
    ///
    /// # Returns
    /// An adapter that tracks binding, activation, and request recovery.
    #[inline]
    pub fn new(prepared: Arc<dyn PreparedFsRead>) -> Self {
        Self {
            prepared,
            lifecycle: Mutex::new(PreparedReadLifecycle::default()),
        }
    }
    /// Returns the request while the prepared task remains unbound.
    ///
    /// Call this after submission is rejected before binding to recover the
    /// request. Once bound, ownership belongs to the managed operation.
    ///
    /// # Returns
    /// `Some(request)` when the task is unbound and still owns its request;
    /// `None` after binding, after a poisoned lifecycle lock, or when the
    /// provider has no unclaimed request.
    pub fn take_request(&self) -> Option<FsOwnedReadRequest> {
        let is_unclaimed = self
            .lifecycle
            .lock()
            .map(|state| state.bound.is_none())
            .unwrap_or(false);
        is_unclaimed.then(|| self.prepared.take_unclaimed_request()).flatten()
    }
}
impl OperationControl for PreparedFsReadAdapter {
    /// Forwards a stop request to the provider task.
    ///
    /// # Parameters
    /// - `r`: Reason the managed operation is stopping.
    fn request_stop(&self, r: StopReason) {
        self.prepared.request_stop(r);
    }
    /// Returns the provider task's current child lifecycle snapshot.
    ///
    /// # Returns
    /// A point-in-time snapshot of the provider task's lifecycle.
    fn snapshot(&self) -> ChildSnapshot {
        self.prepared.snapshot()
    }
    /// Polls the provider task's non-consuming outcome summary.
    ///
    /// # Parameters
    /// - `c`: Poll context used to register the current task for wakeup.
    ///
    /// # Returns
    /// `Pending` until a summary is available, then `Ready` with that summary.
    fn poll_outcome(&self, c: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
        self.prepared.poll_outcome(c)
    }
    /// Polls until the provider task releases its operation resources.
    ///
    /// # Parameters
    /// - `c`: Poll context used to register the current task for wakeup.
    ///
    /// # Returns
    /// `Pending` while resources remain in use, then `Ready` with the release
    /// record.
    fn poll_release(&self, c: &mut Context<'_>) -> Poll<ChildRelease> {
        self.prepared.poll_release(c)
    }
    /// Polls for the provider task's stable settlement report.
    ///
    /// # Parameters
    /// - `c`: Poll context used to register the current task for wakeup.
    ///
    /// # Returns
    /// `Pending` until settlement completes, then `Ready` with a shared report.
    fn poll_settlement(&self, c: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
        self.prepared.poll_settlement(c)
    }
}
impl PreparedExternalOperation<Vec<u8>, FsOperationFailure> for PreparedFsReadAdapter {
    /// Associates this provider task with its unique managed operation.
    ///
    /// # Parameters
    /// - `id`: Stable ID assigned by the operation manager.
    ///
    /// # Returns
    /// `Ok(())` after the provider accepts the binding.
    ///
    /// # Errors
    /// Returns [`ExternalBindError::AlreadyBound`] if the adapter has already
    /// been bound, activated, or abandoned, or propagates the provider error.
    fn bind(&self, id: OperationId) -> Result<(), ExternalBindError> {
        let mut state = self.lifecycle.lock().map_err(|_| ExternalBindError::AlreadyBound)?;
        if state.bound.is_some() || state.abandoned || state.activated {
            return Err(ExternalBindError::AlreadyBound);
        }
        self.prepared.bind(id)?;
        state.bound = Some(id);
        Ok(())
    }
    /// Activates the bound provider task in the supplied runtime scope.
    ///
    /// # Parameters
    /// - `scope`: Runtime scope supplied for the submitted operation.
    ///
    /// # Returns
    /// `Ok(())` after activation is delegated to the provider.
    ///
    /// # Errors
    /// Returns an execution failure if the task was previously activated or
    /// abandoned, or if provider activation fails.
    ///
    /// # Panics
    /// Panics if the local lifecycle mutex is poisoned.
    fn activate(self: Arc<Self>, scope: OperationScope) -> Result<(), ExecutionFailure> {
        {
            let mut state = self.lifecycle.lock().expect("prepared read lifecycle mutex poisoned");
            if (state.activated || state.abandoned)
                && let Some(operation_id) = state.bound
            {
                return Err(ExecutionFailure {
                    operation_id,
                    summary: DiagnosticSummary::new("prepared read was already activated or abandoned"),
                    source: None,
                });
            }
            state.activated = true;
        }
        Arc::clone(&self.prepared).activate(scope)
    }
    /// Abandons a bound task that has not started activation.
    ///
    /// Repeated calls and calls before binding do not abandon the provider
    /// task; the provider is notified only for the first eligible call.
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
    /// Polls for and consumes the read result once the provider completes.
    ///
    /// # Parameters
    /// - `c`: Poll context used to register the current task for wakeup.
    ///
    /// # Returns
    /// `Pending` while the provider is running, then `Ready` with the single
    /// operation outcome.
    fn poll_take_outcome(&self, c: &mut Context<'_>) -> Poll<OperationOutcome<Vec<u8>, FsOperationFailure>> {
        self.prepared.poll_take_outcome(c)
    }
}
