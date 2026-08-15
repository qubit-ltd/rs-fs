// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Server-side copy preference.

/// Preference for provider-native server-side copy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerSidePreference {
    /// Prefer server-side copy and allow fallback.
    Prefer,
    /// Require server-side copy.
    Require,
    /// Disable server-side copy.
    Disable,
}

impl Default for ServerSidePreference {
    /// Disables server-side copy by default.
    #[inline]
    fn default() -> Self {
        Self::Disable
    }
}
