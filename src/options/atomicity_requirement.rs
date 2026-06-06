// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Atomicity requirement used by rename and persist operations.

/// Atomicity requirement for operations that may be downgraded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicityRequirement {
    /// Use the strongest available implementation but allow downgrade.
    BestEffort,
    /// Require atomic behavior or fail with `UnsupportedOperation`.
    Required,
}

impl Default for AtomicityRequirement {
    /// Uses best-effort atomicity by default.
    #[inline]
    fn default() -> Self {
        Self::BestEffort
    }
}
