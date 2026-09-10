// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private resource types owned by the facade policy layer.

mod byte_budget;
mod file_system_resource;

pub(crate) use byte_budget::ByteBudget;
pub(crate) use file_system_resource::FileSystemResource;

mod recovery_cleanup_guard;
pub(crate) use recovery_cleanup_guard::RecoveryCleanupGuard;
