// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use super::FsEffectObserver;
use crate::execution::FsOperation;

/// Mutation handle paired with its independent effect observer.
///
/// # Type Parameters
/// `T` is the successful output produced by the managed operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectSnapshot;
/// use qubit_fs::FsMutationOperation;
///
/// fn inspect<T: Send + 'static>(operation: &FsMutationOperation<T>) {
///     let snapshot = operation.effect.snapshot();
///     assert!(matches!(snapshot, FsEffectSnapshot::Pending | FsEffectSnapshot::Final(_)));
/// }
/// ```
pub struct FsMutationOperation<T: Send + 'static> {
    /// Managed operation handle.
    pub operation: FsOperation<T>,
    /// Independent mutation effect port.
    pub effect: FsEffectObserver,
}
