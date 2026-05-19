/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Copy progress collection policy.

/// Progress collection policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressPolicy {
    /// Do not collect progress beyond final provider output.
    None,
    /// Collect final counts only.
    CountOnly,
    /// Request detailed progress reporting when supported.
    Detailed,
}

impl Default for ProgressPolicy {
    /// Collects final counts by default.
    #[inline]
    fn default() -> Self {
        Self::CountOnly
    }
}
