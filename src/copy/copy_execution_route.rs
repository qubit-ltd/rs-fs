// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Static copy execution route.

/// The route an already-validated copy may take.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CopyExecutionRoute {
    /// Try the provider primitive, then use the stream fallback on decline.
    ProviderThenStream,
    /// Only the provider primitive can satisfy the request.
    ProviderOnly,
    /// Only the stream fallback is available.
    StreamOnly,
}
