/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
//! Path semantics exposed by filesystem implementations.

/// Provider path semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathSemantics {
    /// Hierarchical directory semantics.
    Hierarchical,
    /// Object-key or prefix semantics.
    ObjectKey,
    /// Provider-specific semantics.
    ProviderSpecific,
}

impl Default for PathSemantics {
    /// Uses hierarchical path semantics by default.
    #[inline]
    fn default() -> Self {
        Self::Hierarchical
    }
}
