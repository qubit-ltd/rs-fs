// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated rename request.

use crate::path::Path;
use crate::spi::ResolvedRenameOptions;

/// A facade-created rename request.
pub struct RenameRequest<'a> {
    /// Validated source path.
    source: &'a Path,
    /// Validated destination path.
    target: &'a Path,
    /// Facade-resolved rename policy.
    options: ResolvedRenameOptions,
}

impl<'a> RenameRequest<'a> {
    /// Creates this request inside the facade boundary.
    ///
    /// # Parameters
    /// - `source`: Validated source path.
    /// - `target`: Validated destination path.
    /// - `options`: Facade-resolved rename options.
    ///
    /// # Returns
    /// A provider rename request borrowing both paths.
    #[allow(dead_code)]
    #[inline]
    pub(crate) const fn new(source: &'a Path, target: &'a Path, options: ResolvedRenameOptions) -> Self {
        Self {
            source,
            target,
            options,
        }
    }

    /// Returns the source path.
    ///
    /// # Returns
    /// The validated source path.
    #[inline(always)]
    #[must_use]
    pub const fn source(&self) -> &'a Path {
        self.source
    }

    /// Returns the target path.
    ///
    /// # Returns
    /// The validated destination path.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &'a Path {
        self.target
    }

    /// Returns resolved options.
    ///
    /// # Returns
    /// The immutable facade-resolved rename options.
    #[inline(always)]
    #[must_use]
    pub const fn options(&self) -> &ResolvedRenameOptions {
        &self.options
    }
}
