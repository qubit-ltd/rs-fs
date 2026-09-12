// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-neutral user metadata.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;

/// An ordered string-to-string metadata map with safe structural formatting.
///
/// # Examples
///
/// ```
/// use qubit_fs::metadata::UserMetadata;
///
/// let metadata = UserMetadata::new().with("content-language", "en")?;
/// assert_eq!(Some("en"), metadata.get("content-language"));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Default, Eq, PartialEq)]
pub struct UserMetadata(
    /// Ordered metadata pairs retained without automatic value formatting.
    BTreeMap<String, String>,
);

impl UserMetadata {
    /// Creates empty metadata.
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Adds one metadata pair.
    ///
    /// # Errors
    /// Returns an invalid-options error when the key resembles credential
    /// material.
    pub fn with(mut self, key: &str, value: &str) -> Result<Self, FsError> {
        if crate::uri::query_pair_is_sensitive(key) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Other,
                "credential-like metadata keys are forbidden",
            ));
        }
        self.0.insert(key.to_owned(), value.to_owned());
        Ok(self)
    }

    /// Returns the value associated with a key.
    #[must_use]
    #[inline]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Returns whether the map contains no metadata pairs.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns whether a metadata key is present.
    #[must_use]
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Returns an iterator over metadata pairs.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value.as_str()))
    }
}

impl Debug for UserMetadata {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter
            .debug_struct("UserMetadata")
            .field("keys", &self.0.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::UserMetadata;

    #[test]
    fn metadata_debug_and_iteration_are_executed_at_runtime() {
        let metadata = UserMetadata::new()
            .with("provider", "test")
            .expect("ordinary metadata key should be accepted");
        assert_eq!(metadata.get("provider"), Some("test"));
        assert!(metadata.contains_key("provider"));
        assert_eq!(metadata.iter().collect::<Vec<_>>(), vec![("provider", "test")]);
        assert!(format!("{metadata:?}").contains("UserMetadata"));
    }
}
