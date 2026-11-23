// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::panic::AssertUnwindSafe;
use std::panic::catch_unwind;
use std::panic::resume_unwind;
use std::sync::Mutex;

use qubit_operation::ExternalBindError;
use qubit_operation::OperationId;

/// Binding and lifecycle flags shared by prepared adapter wrappers.
#[derive(Default)]
pub(in crate::execution) struct PreparedReadLifecycle {
    /// Operation ID assigned when a prepared adapter is bound.
    pub(in crate::execution) bound: Option<OperationId>,
    /// Whether provider binding currently owns the claim transition.
    pub(in crate::execution) binding: bool,
    /// Whether request recovery currently owns the competing transition.
    pub(in crate::execution) recovering_request: bool,
    /// Whether request recovery already transferred ownership to the caller.
    pub(in crate::execution) request_recovered: bool,
    /// Whether the prepared provider task has started.
    pub(in crate::execution) activated: bool,
    /// Whether the prepared provider task was abandoned before activation.
    pub(in crate::execution) abandoned: bool,
}

/// Claims the adapter before calling provider code, then commits or rolls back.
///
/// # Parameters
/// - `lifecycle`: Shared state guarding this adapter's ownership transitions.
/// - `id`: Operation identifier assigned to the successful binding.
/// - `bind`: Provider binding callback invoked outside the lifecycle lock.
///
/// # Returns
/// `Ok(())` after binding commits, or the binding error after the claim rolls
/// back.
pub(in crate::execution) fn bind_prepared(
    lifecycle: &Mutex<PreparedReadLifecycle>,
    id: OperationId,
    bind: impl FnOnce() -> Result<(), ExternalBindError>,
) -> Result<(), ExternalBindError> {
    {
        let mut state = lifecycle.lock().map_err(|_| ExternalBindError::AlreadyBound)?;
        if state.bound.is_some()
            || state.binding
            || state.recovering_request
            || state.request_recovered
            || state.activated
            || state.abandoned
        {
            return Err(ExternalBindError::AlreadyBound);
        }
        state.binding = true;
    }

    let result = catch_unwind(AssertUnwindSafe(bind));
    let mut state = lifecycle.lock().map_err(|_| ExternalBindError::AlreadyBound)?;
    state.binding = false;
    match result {
        Ok(Ok(())) => {
            state.bound = Some(id);
            Ok(())
        }
        Ok(Err(error)) => Err(error),
        Err(payload) => {
            drop(state);
            resume_unwind(payload)
        }
    }
}

/// Takes the provider request only after reserving the competing transition.
///
/// # Type Parameters
/// - `T`: Provider request type recovered from the adapter.
///
/// # Parameters
/// - `lifecycle`: Shared state guarding this adapter's ownership transitions.
/// - `take`: One-shot callback that removes the unclaimed provider request.
///
/// # Returns
/// The request if recovery wins the transition; otherwise `None`.
pub(in crate::execution) fn take_unclaimed_request<T>(
    lifecycle: &Mutex<PreparedReadLifecycle>,
    take: impl FnOnce() -> Option<T>,
) -> Option<T> {
    {
        let mut state = lifecycle.lock().ok()?;
        if state.bound.is_some()
            || state.binding
            || state.recovering_request
            || state.request_recovered
            || state.activated
            || state.abandoned
        {
            return None;
        }
        state.recovering_request = true;
    }

    let result = catch_unwind(AssertUnwindSafe(take));
    if let Ok(mut state) = lifecycle.lock() {
        state.recovering_request = false;
        if matches!(&result, Ok(Some(_))) {
            state.request_recovered = true;
        }
    }
    match result {
        Ok(request) => request,
        Err(payload) => resume_unwind(payload),
    }
}
