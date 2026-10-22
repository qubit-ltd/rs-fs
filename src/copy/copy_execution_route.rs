// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Static copy execution route.

/// Selects the execution strategy for an already-validated copy.
///
/// `ProviderThenStream` tries the provider's copy primitive first and falls
/// back to streaming if the provider declines. `ProviderOnly` requires the
/// provider primitive, while `StreamOnly` uses the stream fallback directly.
/// These routes describe strategy selection; they do not guarantee that the
/// copy itself will succeed.
///
/// # Examples
///
/// ```
/// use qubit_fs::copy::CopyExecutionRoute;
///
/// let route = CopyExecutionRoute::ProviderThenStream;
/// assert_eq!(route, CopyExecutionRoute::ProviderThenStream);
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
#[non_exhaustive]
pub enum CopyExecutionRoute {
    /// Try the provider primitive, then use the stream fallback if it declines.
    ProviderThenStream,
    /// Require the provider primitive; do not use the stream fallback.
    ProviderOnly,
    /// Use the stream fallback without trying the provider primitive.
    StreamOnly,
}
