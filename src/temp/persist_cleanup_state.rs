// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Cleanup state reported after temporary-resource persistence.

/// State of the private temporary container after publication succeeds.
#[must_use]
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistCleanupState {
    /// The private temporary container was removed successfully.
    Complete,
    /// Publication succeeded, but the private temporary container remains.
    ResidualTemporaryContainer,
}
