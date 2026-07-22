// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Validated metadata that is safe for automatic structural formatting.

use std::fmt::{
    Debug,
    Formatter,
    Result as FmtResult,
};
use std::ops::Deref;

use qubit_metadata::Metadata;
use serde_json::Value as JsonValue;

use crate::path::is_sensitive_key;
use crate::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};

/// Metadata whose structural keys have passed credential-sensitivity checks.
///
/// The validation covers top-level keys, keys in string maps, and JSON object
/// keys at every depth, including objects nested inside arrays. Scalar values
/// are intentionally not classified: callers must still avoid putting secrets
/// under innocuous keys. This type's [`Debug`] implementation prints keys only
/// and never automatically exposes values.
///
/// The inner [`Metadata`] is not mutably exposed, so every value of this type
/// retains the validation invariant after construction.
#[derive(Clone, PartialEq, Default)]
pub struct NonSensitiveMetadata(Metadata);

impl NonSensitiveMetadata {
    /// Creates empty validated metadata.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self(Metadata::new())
    }

    /// Validates metadata for a specific filesystem operation.
    pub(crate) fn try_from_with_context(
        metadata: Metadata,
        operation: FsOperation,
        message: &str,
    ) -> FsResult<Self> {
        if metadata_contains_sensitive_key(&metadata) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                operation,
                message,
            ));
        }
        Ok(Self(metadata))
    }

    /// Returns the validated metadata without mutable access.
    #[inline(always)]
    #[must_use]
    pub const fn as_metadata(&self) -> &Metadata {
        &self.0
    }

    /// Consumes this wrapper and returns the underlying metadata.
    #[inline]
    #[must_use]
    pub fn into_metadata(self) -> Metadata {
        self.0
    }
}

impl TryFrom<Metadata> for NonSensitiveMetadata {
    type Error = FsError;

    /// Validates an arbitrary metadata value.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::InvalidOptions`] when a top-level key or a key
    /// nested in a string map or JSON object resembles credential material.
    #[inline]
    fn try_from(metadata: Metadata) -> Result<Self, Self::Error> {
        Self::try_from_with_context(
            metadata,
            FsOperation::Other,
            "credential-like metadata keys are forbidden",
        )
    }
}

impl AsRef<Metadata> for NonSensitiveMetadata {
    #[inline]
    fn as_ref(&self) -> &Metadata {
        self.as_metadata()
    }
}

impl Deref for NonSensitiveMetadata {
    type Target = Metadata;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_metadata()
    }
}

impl From<NonSensitiveMetadata> for Metadata {
    #[inline]
    fn from(metadata: NonSensitiveMetadata) -> Self {
        metadata.into_metadata()
    }
}

impl Debug for NonSensitiveMetadata {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let keys: Vec<_> = self.0.keys().collect();
        formatter
            .debug_struct("NonSensitiveMetadata")
            .field("keys", &keys)
            .finish()
    }
}

/// Returns whether metadata contains a credential-like key at any supported
/// structural depth.
fn metadata_contains_sensitive_key(metadata: &Metadata) -> bool {
    metadata.iter().any(|(key, value)| {
        is_sensitive_key(key)
            || value
                .get_string_map_ref()
                .is_ok_and(|map| map.keys().any(|key| is_sensitive_key(key)))
            || value.get_json_ref().is_ok_and(json_contains_sensitive_key)
    })
}

/// Iteratively checks JSON object keys, including objects inside arrays.
fn json_contains_sensitive_key(root: &JsonValue) -> bool {
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        match value {
            JsonValue::Object(object) => {
                for (key, child) in object {
                    if is_sensitive_key(key) {
                        return true;
                    }
                    pending.push(child);
                }
            }
            JsonValue::Array(array) => pending.extend(array),
            _ => {}
        }
    }
    false
}
