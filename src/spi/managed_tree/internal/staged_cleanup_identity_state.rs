// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Mutex-protected state shared by the staged cleanup identity gate.

use std::task::Waker;

use qubit_operation::ResourceId;

/// Published runtime identity and the cleanup task waiting for it.
#[derive(Default)]
pub(super) struct StagedCleanupIdentityState {
    /// Resource identity assigned by the operation resource scope.
    pub(super) id: Option<ResourceId>,
    /// Most recent task waker registered by the identity wait future.
    pub(super) waker: Option<Waker>,
}
