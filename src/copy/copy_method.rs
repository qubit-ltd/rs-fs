// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy method model.

/// Method used to complete a copy operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CopyMethod {
    /// Provider-native copy.
    Native,
    /// Storage-level clone or reflink copy.
    Clone,
    /// Server-side copy.
    ServerSide,
    /// Client-side stream copy.
    Streamed,
    /// Mixed copy strategy.
    Mixed,
}
