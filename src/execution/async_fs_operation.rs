// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem operation handle alias.

use qubit_operation::AsyncOperationHandle;

use super::FsOperationFailure;

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
