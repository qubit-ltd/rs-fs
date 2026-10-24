// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Opaque provider resource versions.

use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

/// Opaque version, generation, or ETag reported by a provider.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::ResourceVersion;
///
/// let version = ResourceVersion::new("v1");
/// assert_eq!("v1", version.as_str());
/// ```
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
    #[must_use]
    #[inline]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the provider-defined version text.
    ///
    /// # Returns
    /// The borrowed version text.
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ResourceVersion {
    /// Writes the provider-defined version text without changing its contents.
    ///
    /// # Parameters
    /// - `formatter`: The formatting destination.
    ///
    /// # Returns
    /// An error if writing to the formatting destination fails.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for ResourceVersion {
    /// Borrows the provider-defined version text as a string slice.
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for ResourceVersion {
    /// Creates a resource version from borrowed provider-defined text.
    ///
    /// # Parameters
    /// - `value`: The version text to copy into the resource version.
    ///
    /// # Returns
    /// A resource version containing `value` exactly.
    #[inline]
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ResourceVersion {
    /// Creates a resource version from owned provider-defined text.
    ///
    /// # Parameters
    /// - `value`: The version text to move into the resource version.
    ///
    /// # Returns
    /// A resource version containing `value` exactly.
    #[inline]
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
