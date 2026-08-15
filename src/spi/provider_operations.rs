// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- exercised through the public provider
// property contract tests.
//! Compact immutable sets of provider operation entry points.

use super::ProviderOperation;

/// Immutable set of concrete operation entry points implemented by a provider.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderOperations {
    /// Bit flags indexed by [`ProviderOperation`] discriminants.
    bits: u128,
}

impl ProviderOperations {
    /// Creates an empty provider-operation set.
    ///
    /// # Returns
    /// A set containing no provider operation entry points.
    #[inline(always)]
    #[must_use]
    pub const fn new() -> Self {
        Self { bits: 0 }
    }

    /// Returns a copy containing `operation`.
    ///
    /// # Parameters
    /// - `operation`: Provider entry point to insert.
    ///
    /// # Returns
    /// The updated immutable operation set.
    #[inline(always)]
    #[must_use]
    pub const fn with(mut self, operation: ProviderOperation) -> Self {
        self.bits |= 1_u128 << operation as u8;
        self
    }

    /// Returns whether the provider implements `operation`.
    ///
    /// # Parameters
    /// - `operation`: Provider entry point to query.
    ///
    /// # Returns
    /// `true` when the operation is present in this snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn supports(&self, operation: ProviderOperation) -> bool {
        self.bits & (1_u128 << operation as u8) != 0
    }
}
