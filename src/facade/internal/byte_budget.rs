// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem byte-budget specialization.

use qubit_budget::ResourceBudget;

use super::FileSystemResource;

/// Tracks facade I/O against an inclusive byte limit for one resource class.
///
/// The resource type distinguishes read bytes from bytes accepted by a
/// destination writer. The `u64` quantity is always a byte count, allowing
/// facade operations to account for cumulative usage without mixing resource
/// categories.
pub(crate) type ByteBudget = ResourceBudget<FileSystemResource, u64>;
