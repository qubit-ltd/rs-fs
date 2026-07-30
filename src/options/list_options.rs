// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Directory listing options.

use crate::{
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    RelativePath,
};

/// Options controlling directory or prefix listing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListOptions {
    /// Whether listing should recurse into child containers.
    pub recursive: bool,
    /// Whether symbolic links should be followed.
    pub follow_symlinks: bool,
    /// Whether entries should include metadata when available.
    pub include_metadata: bool,
    /// Optional provider page size hint.
    pub page_size: Option<usize>,
    /// Optional lexical prefix filter relative to the requested list root.
    ///
    /// The filter uses canonical `/`-separated relative paths. For example,
    /// listing `/root` with `prefix: Some("nested/item")` matches
    /// `/root/nested/item`, while `prefix: Some("item")` only matches an
    /// immediate child named `item`.
    pub prefix: Option<String>,
}

impl ListOptions {
    /// Validates pagination and canonical provider-facing prefix values.
    ///
    /// # Errors
    ///
    /// Returns an invalid-options error when the page size is zero or the
    /// prefix is not a canonical relative path.
    pub fn validate(&self) -> FsResult<()> {
        if self.page_size == Some(0) {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "list page size must be greater than zero",
            ));
        }
        if let Some(prefix) = self.prefix.as_deref() {
            let parsed = RelativePath::parse(prefix).map_err(|_| {
                FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list prefix must be a canonical relative path",
                )
            })?;
            if parsed.as_str() != prefix {
                return Err(FsError::new(
                    FsErrorKind::InvalidOptions,
                    FsOperation::List,
                    "list prefix must be a canonical relative path",
                ));
            }
        }
        Ok(())
    }
}
