// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Execution bindings and owned-operation contracts.

mod async_fs_operation;
mod effect_observer;
mod failure;
mod fs_execution_binding;
mod fs_operation;
mod fs_operation_failure;
mod fs_operation_failure_kind;
mod fs_prepare_failure;
mod fs_prepare_failure_kind;
mod fs_submit_failure;
mod internal;
mod prepared_adapter;
mod request;
pub use async_fs_operation::AsyncFsOperation;
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
pub use fs_operation::FsOperation;
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
