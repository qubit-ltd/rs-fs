// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Adapter module for prepared filesystem provider operations.

/// Generates a typed wrapper for one object-safe prepared provider task.
///
/// Each generated adapter shares the request lifecycle state and delegates
/// operation control, binding, activation, abandonment, and outcome polling to
/// the provider object.
macro_rules! make_prepared_adapter {
    ($name:ident, $prepared:ident, $request:ty, $output:ty) => {
        #[doc = concat!(
                            "Sized adapter for the corresponding object-safe prepared provider task.\n\n",
                            "A prepared task is supplied by its provider during operation preparation.\n\n",
                            "# Examples\n\n",
                            "```rust\n",
                            "use std::sync::Arc;\n",
                            "use qubit_fs::execution::", stringify!($name), ";\n",
                            "use qubit_fs::spi::", stringify!($prepared), ";\n\n",
                            "fn inspect(prepared: Arc<dyn ", stringify!($prepared), ">) {\n",
                            "    let adapter = ", stringify!($name), "::new(prepared);\n",
                            "    let _unclaimed_request = adapter.take_request();\n",
                            "}\n",
                            "```"
                        )]
        #[must_use = "retain the adapter until its provider task is bound or abandoned"]
        pub struct $name {
            /// Object-safe provider task retained until it is bound or abandoned.
            prepared: Arc<dyn crate::spi::$prepared>,
            /// Binding, activation, and abandonment state shared with this adapter.
            lifecycle: Mutex<PreparedReadLifecycle>,
        }
        impl $name {
            /// Creates an adapter for an unclaimed provider task.
            ///
            /// # Parameters
            /// - `prepared`: Provider task that owns the request before submission.
            ///
            /// # Returns
            /// An adapter retaining the task until it is bound, activated, or
            /// abandoned.
            #[inline]
            pub fn new(prepared: Arc<dyn crate::spi::$prepared>) -> Self {
                Self {
                    prepared,
                    lifecycle: Mutex::new(PreparedReadLifecycle::default()),
                }
            }
            /// Returns the original request while the task remains unclaimed.
            ///
            /// # Returns
            /// `Some(request)` while the prepared task has not been bound and still
            /// owns its request; `None` after binding or when the provider has no
            /// unclaimed request.
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
            /// Forwards a stop request to the prepared provider task.
            fn request_stop(&self, reason: StopReason) {
                self.prepared.request_stop(reason);
            }
            /// Returns the provider task's current child lifecycle snapshot.
            fn snapshot(&self) -> ChildSnapshot {
                self.prepared.snapshot()
            }
            /// Polls the provider task's non-consuming outcome summary.
            fn poll_outcome(&self, cx: &mut Context<'_>) -> Poll<ChildOutcomeSummary> {
                self.prepared.poll_outcome(cx)
            }
            /// Polls until the provider task releases its operation resources.
            fn poll_release(&self, cx: &mut Context<'_>) -> Poll<ChildRelease> {
                self.prepared.poll_release(cx)
            }
            /// Polls for the provider task's stable settlement report.
            fn poll_settlement(&self, cx: &mut Context<'_>) -> Poll<Arc<SettlementReport>> {
                self.prepared.poll_settlement(cx)
            }
        }
        impl PreparedExternalOperation<$output, FsOperationFailure> for $name {
            /// Binds the provider task to its unique managed operation.
            ///
            /// # Parameters
            /// - `id`: Stable ID assigned by the operation manager.
            ///
            /// # Returns
            /// `Ok(())` after the provider accepts the binding.
            ///
            /// # Errors
            /// Returns [`ExternalBindError::AlreadyBound`] if this adapter cannot
            /// be bound again, or propagates the provider binding error.
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
            /// Activates the bound task and transfers execution to the provider.
            ///
            /// # Parameters
            /// - `scope`: Runtime scope supplied for the submitted operation.
            ///
            /// # Returns
            /// `Ok(())` when provider activation succeeds.
            ///
            /// # Errors
            /// Returns an execution failure if the adapter was already activated
            /// or abandoned, or if the provider cannot activate the task.
            fn activate(self: Arc<Self>, scope: OperationScope) -> Result<(), ExecutionFailure> {
                let mut state = self
                    .lifecycle
                    .lock()
                    .expect("prepared operation lifecycle mutex poisoned");
                if state.activated || state.abandoned {
                    if let Some(operation_id) = state.bound {
                        return Err(ExecutionFailure {
                            operation_id,
                            summary: DiagnosticSummary::new("prepared operation was already activated or abandoned"),
                            source: None,
                        });
                    }
                }
                state.activated = true;
                drop(state);
                Arc::clone(&self.prepared).activate(scope)
            }
            /// Abandons a bound task that has not started execution.
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
            /// Polls for and takes the provider task's typed operation outcome.
            fn poll_take_outcome(&self, cx: &mut Context<'_>) -> Poll<OperationOutcome<$output, FsOperationFailure>> {
                self.prepared.poll_take_outcome(cx)
            }
        }
    };
}

mod prepared_fs_copy_adapter;
mod prepared_fs_read_adapter;
mod prepared_fs_rename_adapter;
mod prepared_fs_tree_publish_adapter;
mod prepared_fs_write_adapter;

pub use prepared_fs_copy_adapter::PreparedFsCopyAdapter;
pub use prepared_fs_read_adapter::PreparedFsReadAdapter;
pub use prepared_fs_rename_adapter::PreparedFsRenameAdapter;
pub use prepared_fs_tree_publish_adapter::PreparedFsTreePublishAdapter;
pub use prepared_fs_write_adapter::PreparedFsWriteAdapter;
