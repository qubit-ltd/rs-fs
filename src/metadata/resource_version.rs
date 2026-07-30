// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Opaque provider resource versions.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

/// Opaque version, generation, or ETag reported by a provider.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResourceVersion(
    /// Provider-defined opaque version text.
    String,
);

impl ResourceVersion {
    /// Creates an opaque resource version.
    ///
    /// # Parameters
    /// - `value`: Provider-defined version text.
    ///
    /// # Returns
    /// A resource version preserving `value` exactly.
    #[inline]
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the provider-defined version text.
    ///
    /// # Returns
    /// The borrowed version text.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ResourceVersion {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for ResourceVersion {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for ResourceVersion {
    #[inline]
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ResourceVersion {
    #[inline]
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
