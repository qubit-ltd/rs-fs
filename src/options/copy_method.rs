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
    /// Local provider-native copy.
    Local,
    /// Server-side copy.
    ServerSide,
    /// Client-side stream copy.
    Stream,
    /// Mixed copy strategy.
    Mixed,
}
