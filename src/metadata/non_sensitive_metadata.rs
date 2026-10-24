// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated metadata that is safe for automatic structural formatting.

use crate::metadata::UserMetadata;

/// Flat metadata whose keys have passed credential-sensitivity checks.
///
/// [`UserMetadata`] stores ordered string pairs and rejects credential-like
/// keys when each pair is added. This type's [`Debug`] implementation prints
/// keys only and never automatically exposes values.
///
/// The inner [`UserMetadata`] is not mutably exposed, so every value of this
/// type retains the wrapper invariant after construction.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::{NonSensitiveMetadata, UserMetadata};
///
/// let safe = NonSensitiveMetadata::from(
///     UserMetadata::new().with("content-language", "en")?,
/// );
/// assert_eq!(Some("en"), safe.get("content-language"));
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Default)]
pub struct NonSensitiveMetadata(
    /// Validated metadata whose keys do not resemble credential material.
    UserMetadata,
);

impl NonSensitiveMetadata {
    /// Creates empty validated metadata.
    ///
    /// # Returns
    /// An empty metadata wrapper.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(UserMetadata::new())
    }

    /// Returns the validated metadata without mutable access.
    ///
    /// # Returns
    /// A shared reference to the wrapped metadata.
    #[inline]
    #[must_use]
    pub const fn as_metadata(&self) -> &UserMetadata {
        &self.0
    }

    /// Consumes this wrapper and returns the underlying metadata.
    ///
    /// # Returns
    /// The validated metadata previously held by this wrapper.
    #[inline]
    #[must_use]
    pub fn into_metadata(self) -> UserMetadata {
        self.0
    }

    /// Returns whether the wrapped map contains no metadata pairs.
    ///
    /// # Returns
    /// `true` when the wrapped map is empty, or `false` otherwise.
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns whether a metadata key is present.
    ///
    /// # Parameters
    /// `key` is the metadata name to look up.
    ///
    /// # Returns
    /// `true` when `key` is present, or `false` otherwise.
    #[must_use]
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.contains_key(key)
    }

    /// Returns the value associated with a metadata key.
    ///
    /// # Parameters
    /// `key` is the metadata name to look up.
    ///
    /// # Returns
    /// `Some` with the value when `key` is present, or `None` otherwise.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)
    }
}

impl From<UserMetadata> for NonSensitiveMetadata {
    /// Wraps arbitrary user metadata.
    ///
    /// # Parameters
    /// `metadata` is the metadata value to wrap.
    ///
    /// # Returns
    /// A wrapper retaining the supplied metadata.
    #[inline]
    fn from(metadata: UserMetadata) -> Self {
        Self(metadata)
    }
}

impl AsRef<UserMetadata> for NonSensitiveMetadata {
    /// Borrows the validated metadata without exposing mutable access.
    ///
    /// # Returns
    /// A shared reference to the wrapped metadata.
    #[inline]
    fn as_ref(&self) -> &UserMetadata {
        self.as_metadata()
    }
}

impl From<NonSensitiveMetadata> for UserMetadata {
    /// Consumes the wrapper and returns its validated metadata.
    ///
    /// # Parameters
    /// `metadata` is the validated metadata wrapper to consume.
    ///
    /// # Returns
    /// The metadata previously held by the wrapper.
    #[inline]
    fn from(metadata: NonSensitiveMetadata) -> Self {
        metadata.into_metadata()
    }
}
