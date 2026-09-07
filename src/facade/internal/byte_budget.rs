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

/// A budget that counts filesystem bytes.
pub(crate) type ByteBudget = ResourceBudget<FileSystemResource, u64>;
