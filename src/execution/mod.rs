// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Execution bindings and owned-operation contracts.

use qubit_operation::AsyncOperationHandle;
use qubit_operation::OperationHandle;

mod effect_observer;
mod failure;
mod fs_execution_binding;
mod fs_operation_failure;
mod fs_operation_failure_kind;
mod fs_prepare_failure;
mod fs_prepare_failure_kind;
mod fs_submit_failure;
mod internal;
mod prepared_adapter;
mod request;
pub use effect_observer::AsyncFsMutationOperation;
pub use effect_observer::FsEffectCapacityError;
pub use effect_observer::FsEffectFact;
pub use effect_observer::FsEffectObservationError;
pub use effect_observer::FsEffectObserver;
pub use effect_observer::FsEffectReservation;
pub use effect_observer::FsEffectSlotPool;
pub use effect_observer::FsEffectSnapshot;
pub use effect_observer::FsEffectWait;
pub use effect_observer::FsMutationOperation;
pub use failure::FsOperationFailure;
pub use failure::FsOperationFailureKind;
pub use failure::FsPrepareFailure;
pub use failure::FsPrepareFailureKind;
pub use failure::FsSubmitFailure;
pub use fs_execution_binding::FsExecutionBinding;
pub use prepared_adapter::PreparedFsCopyAdapter;
pub use prepared_adapter::PreparedFsReadAdapter;
pub use prepared_adapter::PreparedFsRenameAdapter;
pub use prepared_adapter::PreparedFsTreePublishAdapter;
pub use prepared_adapter::PreparedFsWriteAdapter;
pub use request::FsOwnedCopyRequest;
pub use request::FsOwnedReadRequest;
pub use request::FsOwnedRenameRequest;
pub use request::FsOwnedTreePublishRequest;
pub use request::FsOwnedWriteRequest;
/// Unique synchronous filesystem operation handle returned after submission.
///
/// # Type Parameters
/// - `T`: The successful output produced by the operation.
/// - `E`: The failure value produced by the operation. Both types must be `Send
///   + 'static` as required by the underlying operation runtime.
///
/// # Examples
///
/// A synchronous handle is returned by an `FsExecutionBinding::submit_*`
/// method.
///
/// ```rust
/// use qubit_fs::FsOperation;
///
/// fn inspect<T: Send + 'static, E: Send + 'static>(operation: &FsOperation<T, E>) {
///     let _operation_id = operation.id();
///     let _observer = operation.observer();
/// }
/// ```
pub type FsOperation<T, E = FsOperationFailure> = OperationHandle<T, E>;
/// Unique asynchronous filesystem operation handle returned after submission.
///
/// # Type Parameters
/// - `T`: The successful output produced by the operation.
/// - `E`: The failure value produced by the operation. Both types must be `Send
///   + 'static` as required by the underlying operation runtime.
///
/// # Examples
///
/// An asynchronous handle is returned by an
/// `FsExecutionBinding::submit_*_async` method.
///
/// ```rust
/// use qubit_fs::AsyncFsOperation;
///
/// fn inspect<T: Send + 'static, E: Send + 'static>(operation: &AsyncFsOperation<T, E>) {
///     let _operation_id = operation.id();
///     let _observer = operation.observer();
/// }
/// ```
pub type AsyncFsOperation<T, E = FsOperationFailure> = AsyncOperationHandle<T, E>;
