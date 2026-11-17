// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::FsEffectObserver;
use crate::execution::AsyncFsOperation;

/// Async mutation handle paired with its independent effect observer.
///
/// # Type Parameters
/// `T` is the successful output produced by the asynchronous managed operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::AsyncFsMutationOperation;
/// use qubit_fs::FsEffectSnapshot;
///
/// fn inspect<T: Send + 'static>(operation: &AsyncFsMutationOperation<T>) {
///     let snapshot = operation.effect.snapshot();
///     assert!(matches!(snapshot, FsEffectSnapshot::Pending | FsEffectSnapshot::Final(_)));
/// }
/// ```
#[must_use = "retain the operation to observe its outcome and mutation effect"]
pub struct AsyncFsMutationOperation<T: Send + 'static> {
    /// Handle for awaiting or consuming the managed operation outcome and
    /// observing its lifecycle.
    pub operation: AsyncFsOperation<T>,
    /// Separate port for inspecting or waiting on the mutation facts; its
    /// state is independent of whether the operation outcome has been read.
    pub effect: FsEffectObserver,
}
