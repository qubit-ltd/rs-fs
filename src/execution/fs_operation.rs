// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous filesystem operation handle alias.

use qubit_operation::OperationHandle;

use super::FsOperationFailure;

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
