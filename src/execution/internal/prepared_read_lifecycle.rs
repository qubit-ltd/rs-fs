// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use qubit_operation::OperationId;

/// Binding and lifecycle flags shared by prepared adapter wrappers.
#[derive(Default)]
pub(in crate::execution) struct PreparedReadLifecycle {
    /// Operation ID assigned when a prepared adapter is bound.
    pub(in crate::execution) bound: Option<OperationId>,
    /// Whether the prepared provider task has started.
    pub(in crate::execution) activated: bool,
    /// Whether the prepared provider task was abandoned before activation.
    pub(in crate::execution) abandoned: bool,
}
