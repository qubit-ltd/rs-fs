// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;
use std::task::Waker;

/// One independently cancellable async waiter registration.
pub(in crate::execution::effect_observer) struct FsEffectWaiter {
    /// Opaque identity retained by the future that owns this registration.
    pub(in crate::execution::effect_observer) id: Arc<()>,
    /// Most recent task waker supplied by that future.
    pub(in crate::execution::effect_observer) waker: Waker,
}
