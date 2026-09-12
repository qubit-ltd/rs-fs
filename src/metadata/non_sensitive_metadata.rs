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
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(UserMetadata::new())
    }

    /// Returns the validated metadata without mutable access.
    #[inline]
    #[must_use]
    pub const fn as_metadata(&self) -> &UserMetadata {
        &self.0
    }

    /// Consumes this wrapper and returns the underlying metadata.
    #[inline]
    #[must_use]
    pub fn into_metadata(self) -> UserMetadata {
        self.0
    }

    /// Returns whether the wrapped map contains no metadata pairs.
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

    /// Returns the value associated with a metadata key.
    #[must_use]
    #[inline]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key)
    }
}

impl From<UserMetadata> for NonSensitiveMetadata {
    /// Wraps arbitrary user metadata.
    #[inline]
    fn from(metadata: UserMetadata) -> Self {
        Self(metadata)
    }
}

impl AsRef<UserMetadata> for NonSensitiveMetadata {
    #[inline]
    fn as_ref(&self) -> &UserMetadata {
        self.as_metadata()
    }
}

impl From<NonSensitiveMetadata> for UserMetadata {
    #[inline]
    fn from(metadata: NonSensitiveMetadata) -> Self {
        metadata.into_metadata()
    }
}

#[cfg(test)]
mod tests {
    use std::hint::black_box;

    use super::NonSensitiveMetadata;
    use crate::metadata::UserMetadata;

    #[test]
    fn metadata_wrapper_contract_is_executed_at_runtime() {
        let constructor: fn() -> NonSensitiveMetadata = black_box(NonSensitiveMetadata::new);
        let as_metadata: fn(&NonSensitiveMetadata) -> &UserMetadata = black_box(NonSensitiveMetadata::as_metadata);
        let into_metadata: fn(NonSensitiveMetadata) -> UserMetadata = black_box(NonSensitiveMetadata::into_metadata);
        let is_empty: fn(&NonSensitiveMetadata) -> bool = black_box(NonSensitiveMetadata::is_empty);
        let contains_key: fn(&NonSensitiveMetadata, &str) -> bool = black_box(NonSensitiveMetadata::contains_key);
        let get: for<'a, 'b> fn(&'a NonSensitiveMetadata, &'b str) -> Option<&'a str> =
            black_box(NonSensitiveMetadata::get);

        let empty = constructor();
        assert!(is_empty(&empty));
        assert!(!contains_key(&empty, "owner"));
        assert_eq!(None, get(&empty, "owner"));
        assert!(as_metadata(&empty).is_empty());

        let user_metadata = UserMetadata::new()
            .with("owner", "storage")
            .expect("safe metadata key must be accepted");
        let wrapped: NonSensitiveMetadata = black_box(user_metadata.into());
        assert!(contains_key(&wrapped, "owner"));
        assert_eq!(Some("storage"), get(&wrapped, "owner"));
        assert_eq!("storage", as_metadata(&wrapped).get("owner").unwrap());
        assert_eq!(Some("storage"), into_metadata(wrapped).get("owner"));
    }
}
