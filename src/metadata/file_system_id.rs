// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Stable configured filesystem identity.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
};

/// Stable identity of one configured filesystem object.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FileSystemId(Box<str>);

impl FileSystemId {
    /// Validates a filesystem identity supplied by a provider.
    ///
    /// # Errors
    ///
    /// Returns [`FsErrorKind::InvalidOptions`] for empty identities or control
    /// characters.
    pub fn new(id: &str) -> FsResult<Self> {
        if id.is_empty() || id.chars().any(char::is_control) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Provider,
                "filesystem id must be non-empty and contain no controls",
            ));
        }
        Ok(Self(id.into()))
    }

    /// Returns the provider-supplied stable identity.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for FileSystemId {
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}
