// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow all -- facade integration tests exercise this API group.
//! Durability requirements for completed filesystem operations.

/// Required storage synchronization strength for an operation outcome.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DurabilityRequirement {
    /// The operation must confirm durable data and namespace publication.
    Required,
    /// Prefer durable publication but report a downgrade in the outcome.
    Preferred,
    /// Do not request an explicit durability guarantee.
    #[default]
    NotRequired,
}
