// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Atomicity requirement used by write, rename, and persist operations.

/// Atomicity contract requested by an operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicityRequirement {
    /// Success must be atomic; unsupported guarantees fail before side effects.
    Required,
    /// Prefer an atomic method but permit a reported non-atomic result.
    Preferred,
    /// Do not require atomicity, although an implementation may still use it.
    NotRequired,
}

impl Default for AtomicityRequirement {
    /// Prefers atomic behavior while permitting an explicit fallback.
    #[inline]
    fn default() -> Self {
        Self::Preferred
    }
}
